// SPDX-License-Identifier: Apache-2.0
//
// P/Invoke surface for the datalogic-c C ABI (v2). Hand-written rather
// than generated — the C ABI has ~40 entry points, so the generator's
// scaffolding (csbindgen + a Rust build step) would cost more in
// developer friction than it saves. If the surface grows past ~50 fns
// reconsider switching to csbindgen.
//
// The v2 contract implemented here (see bindings/c/README.md):
//   - `datalogic_abi_version()` must equal AbiVersion (2) — asserted
//     once at first use by NativeInit.
//   - Byte inputs are (pointer, length) UTF-8, never NUL-terminated;
//     Utf8Input does the managed-string encoding.
//   - Fallible calls return `datalogic_status` and take a trailing
//     `datalogic_error **err`: pass a slot initialised to IntPtr.Zero,
//     and release whatever lands in it via `datalogic_error_free`
//     (DatalogicException.FromNative does both reads and the free).
//   - Session results are BORROWED (ptr, len) into a session-owned
//     buffer, valid until the next call touching that session — copy
//     into a managed string immediately (BorrowedUtf8).
//   - One-shot results are OWNED `datalogic_buf` values — copy then
//     `datalogic_buf_free` (passed BY VALUE; TakeBufUtf8 does both).
//
// All entry points use `LibraryImport` (source-generated P/Invoke), so
// the assembly is NativeAOT-ready out of the box.

using System.Runtime.InteropServices;
using System.Text;

namespace Goplasmatic.Datalogic.Native;

/// <summary>
/// Mirror of the C ABI's `datalogic_status` enum — the coarse,
/// branchable outcome of every fallible native call.
/// </summary>
internal enum DatalogicStatus
{
    Ok = 0,
    InvalidArg = 1,
    Parse = 2,
    Eval = 3,
    TypeMismatch = 4,
    Internal = 5,
}

/// <summary>
/// Mirror of `datalogic_buf`: an owned byte buffer returned by the
/// one-shot entry points. Copy the bytes then release via
/// <see cref="NativeMethods.datalogic_buf_free"/> (by value) — or use
/// <see cref="NativeMethods.TakeBufUtf8"/> which does both.
/// </summary>
[StructLayout(LayoutKind.Sequential)]
internal unsafe struct DatalogicBuf
{
    public byte* Ptr;
    public nuint Len;
    public nuint Cap;
}

/// <summary>
/// Mirror of `datalogic_slice`: a borrowed byte range used by the batch
/// result arrays. Validity follows the owning session's borrow rules —
/// copy before the next call touching the session.
/// </summary>
[StructLayout(LayoutKind.Sequential)]
internal unsafe struct DatalogicSlice
{
    public byte* Ptr;
    public nuint Len;
}

internal static partial class NativeMethods
{
    /// <summary>
    /// The shared-library name the runtime looks up. Resolution:
    /// 1. Whatever <see cref="DllImportResolver"/> returns first (see
    ///    NativeLibraryResolver — falls back to the C ABI's cargo target
    ///    dir for local dev).
    /// 2. Otherwise the standard NuGet `runtimes/&lt;rid&gt;/native/`
    ///    layout populated at packaging time by the release workflow.
    /// </summary>
    internal const string LibraryName = "datalogic_c";

    /// <summary>
    /// The C ABI revision this binding is written against. NativeInit
    /// refuses to run against a native library reporting anything else.
    /// </summary>
    internal const uint AbiVersion = 2;

    // =============== Meta ===============

    [LibraryImport(LibraryName, EntryPoint = "datalogic_abi_version")]
    internal static partial uint datalogic_abi_version();

    [LibraryImport(LibraryName, EntryPoint = "datalogic_version")]
    internal static partial IntPtr datalogic_version();

    [LibraryImport(LibraryName, EntryPoint = "datalogic_buf_free")]
    internal static partial void datalogic_buf_free(DatalogicBuf buf);

    // =============== Engine ===============

    [LibraryImport(LibraryName, EntryPoint = "datalogic_engine_new")]
    internal static partial IntPtr datalogic_engine_new(int templating);

    [LibraryImport(LibraryName, EntryPoint = "datalogic_engine_free")]
    internal static partial void datalogic_engine_free(IntPtr engine);

    [LibraryImport(LibraryName, EntryPoint = "datalogic_engine_compile")]
    internal static unsafe partial DatalogicStatus datalogic_engine_compile(
        IntPtr engine,
        byte* rule_json,
        nuint rule_len,
        out IntPtr out_rule,
        ref IntPtr err);

    [LibraryImport(LibraryName, EntryPoint = "datalogic_engine_apply")]
    internal static unsafe partial DatalogicStatus datalogic_engine_apply(
        IntPtr engine,
        byte* rule_json,
        nuint rule_len,
        byte* data_json,
        nuint data_len,
        out DatalogicBuf @out,
        ref IntPtr err);

    [LibraryImport(LibraryName, EntryPoint = "datalogic_engine_session")]
    internal static partial IntPtr datalogic_engine_session(IntPtr engine);

    [LibraryImport(LibraryName, EntryPoint = "datalogic_engine_traced_session")]
    internal static partial IntPtr datalogic_engine_traced_session(IntPtr engine);

    // =============== Engine builder (config + custom operators) ===============

    [LibraryImport(LibraryName, EntryPoint = "datalogic_engine_builder_new")]
    internal static partial IntPtr datalogic_engine_builder_new();

    [LibraryImport(LibraryName, EntryPoint = "datalogic_engine_builder_free")]
    internal static partial void datalogic_engine_builder_free(IntPtr builder);

    [LibraryImport(LibraryName, EntryPoint = "datalogic_engine_builder_set_templating")]
    internal static partial void datalogic_engine_builder_set_templating(IntPtr builder, int enabled);

    [LibraryImport(LibraryName, EntryPoint = "datalogic_engine_builder_set_config_json")]
    internal static unsafe partial DatalogicStatus datalogic_engine_builder_set_config_json(
        IntPtr builder,
        byte* config_json,
        nuint config_len,
        ref IntPtr err);

    // The v2 callback contract (`datalogic_op_fn`):
    //   int32 fn(const uint8_t *args_json, size_t args_len,
    //            void *user_data, datalogic_op_result *out)
    // The callback writes its outcome through the two setters below and
    // returns 0 for success / non-zero for failure. No allocator
    // crosses the boundary in either direction. The function pointer is
    // produced with `delegate* unmanaged[Cdecl]<byte*, nuint, void*,
    // IntPtr, int>` in EngineBuilder and passed as IntPtr here.
    [LibraryImport(LibraryName, EntryPoint = "datalogic_engine_builder_add_operator")]
    internal static unsafe partial DatalogicStatus datalogic_engine_builder_add_operator(
        IntPtr builder,
        byte* name,
        nuint name_len,
        IntPtr callback,
        IntPtr user_data,
        ref IntPtr err);

    [LibraryImport(LibraryName, EntryPoint = "datalogic_engine_builder_build")]
    internal static partial IntPtr datalogic_engine_builder_build(IntPtr builder);

    [LibraryImport(LibraryName, EntryPoint = "datalogic_op_result_set_json")]
    internal static unsafe partial void datalogic_op_result_set_json(IntPtr @out, byte* json, nuint len);

    [LibraryImport(LibraryName, EntryPoint = "datalogic_op_result_set_error")]
    internal static unsafe partial void datalogic_op_result_set_error(IntPtr @out, byte* msg, nuint len);

    // =============== Data handles ===============

    [LibraryImport(LibraryName, EntryPoint = "datalogic_data_parse")]
    internal static unsafe partial DatalogicStatus datalogic_data_parse(
        byte* json,
        nuint len,
        out IntPtr @out,
        ref IntPtr err);

    [LibraryImport(LibraryName, EntryPoint = "datalogic_data_free")]
    internal static partial void datalogic_data_free(IntPtr data);

    [LibraryImport(LibraryName, EntryPoint = "datalogic_data_allocated_bytes")]
    internal static partial nuint datalogic_data_allocated_bytes(IntPtr data);

    // =============== Rule ===============

    [LibraryImport(LibraryName, EntryPoint = "datalogic_rule_free")]
    internal static partial void datalogic_rule_free(IntPtr rule);

    [LibraryImport(LibraryName, EntryPoint = "datalogic_rule_evaluate")]
    internal static unsafe partial DatalogicStatus datalogic_rule_evaluate(
        IntPtr rule,
        byte* data_json,
        nuint data_len,
        out DatalogicBuf @out,
        ref IntPtr err);

    [LibraryImport(LibraryName, EntryPoint = "datalogic_rule_evaluate_data")]
    internal static unsafe partial DatalogicStatus datalogic_rule_evaluate_data(
        IntPtr rule,
        IntPtr data,
        out DatalogicBuf @out,
        ref IntPtr err);

    // =============== Session ===============

    [LibraryImport(LibraryName, EntryPoint = "datalogic_session_free")]
    internal static partial void datalogic_session_free(IntPtr session);

    [LibraryImport(LibraryName, EntryPoint = "datalogic_session_reset")]
    internal static partial void datalogic_session_reset(IntPtr session);

    [LibraryImport(LibraryName, EntryPoint = "datalogic_session_allocated_bytes")]
    internal static partial nuint datalogic_session_allocated_bytes(IntPtr session);

    [LibraryImport(LibraryName, EntryPoint = "datalogic_session_evaluate")]
    internal static unsafe partial DatalogicStatus datalogic_session_evaluate(
        IntPtr session,
        IntPtr rule,
        byte* data_json,
        nuint data_len,
        out byte* out_ptr,
        out nuint out_len,
        ref IntPtr err);

    [LibraryImport(LibraryName, EntryPoint = "datalogic_session_evaluate_data")]
    internal static unsafe partial DatalogicStatus datalogic_session_evaluate_data(
        IntPtr session,
        IntPtr rule,
        IntPtr data,
        out byte* out_ptr,
        out nuint out_len,
        ref IntPtr err);

    [LibraryImport(LibraryName, EntryPoint = "datalogic_session_evaluate_bool")]
    internal static partial DatalogicStatus datalogic_session_evaluate_bool(
        IntPtr session,
        IntPtr rule,
        IntPtr data,
        out int @out,
        ref IntPtr err);

    [LibraryImport(LibraryName, EntryPoint = "datalogic_session_evaluate_i64")]
    internal static partial DatalogicStatus datalogic_session_evaluate_i64(
        IntPtr session,
        IntPtr rule,
        IntPtr data,
        out long @out,
        ref IntPtr err);

    [LibraryImport(LibraryName, EntryPoint = "datalogic_session_evaluate_f64")]
    internal static partial DatalogicStatus datalogic_session_evaluate_f64(
        IntPtr session,
        IntPtr rule,
        IntPtr data,
        out double @out,
        ref IntPtr err);

    [LibraryImport(LibraryName, EntryPoint = "datalogic_session_evaluate_truthy")]
    internal static partial DatalogicStatus datalogic_session_evaluate_truthy(
        IntPtr session,
        IntPtr rule,
        IntPtr data,
        out int @out,
        ref IntPtr err);

    [LibraryImport(LibraryName, EntryPoint = "datalogic_session_evaluate_batch")]
    internal static unsafe partial DatalogicStatus datalogic_session_evaluate_batch(
        IntPtr session,
        IntPtr rule,
        IntPtr* datas,
        nuint n,
        DatalogicSlice* out_results,
        DatalogicStatus* out_statuses,
        ref IntPtr err);

    [LibraryImport(LibraryName, EntryPoint = "datalogic_session_evaluate_many")]
    internal static unsafe partial DatalogicStatus datalogic_session_evaluate_many(
        IntPtr session,
        IntPtr* rules,
        nuint n,
        IntPtr data,
        DatalogicSlice* out_results,
        DatalogicStatus* out_statuses,
        ref IntPtr err);

    // =============== Traced session ===============

    [LibraryImport(LibraryName, EntryPoint = "datalogic_traced_session_free")]
    internal static partial void datalogic_traced_session_free(IntPtr session);

    [LibraryImport(LibraryName, EntryPoint = "datalogic_traced_session_evaluate")]
    internal static unsafe partial DatalogicStatus datalogic_traced_session_evaluate(
        IntPtr session,
        byte* rule_json,
        nuint rule_len,
        byte* data_json,
        nuint data_len,
        out DatalogicBuf @out,
        ref IntPtr err);

    // =============== Error handles ===============

    [LibraryImport(LibraryName, EntryPoint = "datalogic_error_free")]
    internal static partial void datalogic_error_free(IntPtr err);

    [LibraryImport(LibraryName, EntryPoint = "datalogic_error_status")]
    internal static partial DatalogicStatus datalogic_error_status(IntPtr err);

    [LibraryImport(LibraryName, EntryPoint = "datalogic_error_message")]
    internal static unsafe partial byte* datalogic_error_message(IntPtr err, out nuint len_out);

    [LibraryImport(LibraryName, EntryPoint = "datalogic_error_tag")]
    internal static unsafe partial byte* datalogic_error_tag(IntPtr err, out nuint len_out);

    [LibraryImport(LibraryName, EntryPoint = "datalogic_error_operator")]
    internal static unsafe partial byte* datalogic_error_operator(IntPtr err, out nuint len_out);

    [LibraryImport(LibraryName, EntryPoint = "datalogic_error_path_json")]
    internal static unsafe partial byte* datalogic_error_path_json(IntPtr err, out nuint len_out);

    // =============== Managed helpers ===============

    /// <summary>
    /// Copy an owned <see cref="DatalogicBuf"/> into a managed string
    /// and release it via <see cref="datalogic_buf_free"/> (by value).
    /// Only call with a buf a native call actually filled (status Ok).
    /// </summary>
    internal static unsafe string TakeBufUtf8(DatalogicBuf buf)
    {
        try
        {
            return buf.Ptr == null || buf.Len == 0
                ? string.Empty
                : Encoding.UTF8.GetString(buf.Ptr, checked((int)buf.Len));
        }
        finally
        {
            datalogic_buf_free(buf);
        }
    }

    /// <summary>
    /// Copy borrowed (ptr, len) bytes into a managed string. The caller
    /// must do this before the next call touching the owning session.
    /// </summary>
    internal static unsafe string BorrowedUtf8(byte* ptr, nuint len)
        => ptr == null || len == 0 ? string.Empty : Encoding.UTF8.GetString(ptr, checked((int)len));

    /// <summary>
    /// Marshal a borrowed NUL-terminated UTF-8 C string (library-owned,
    /// never freed). Only used for `datalogic_version`, the one
    /// deliberate NUL-terminated survivor of the v2 contract.
    /// </summary>
    internal static string? BorrowUtf8String(IntPtr ptr)
        => ptr == IntPtr.Zero ? null : Marshal.PtrToStringUTF8(ptr);
}
