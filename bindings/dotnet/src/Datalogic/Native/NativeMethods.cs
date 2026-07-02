// SPDX-License-Identifier: Apache-2.0
//
// P/Invoke surface for the datalogic-c C ABI. Hand-written rather than
// generated — the C ABI has ~17 entry points, so the generator's
// scaffolding (csbindgen + a Rust build step) would cost more in
// developer friction than it saves. If the surface grows past ~50 fns
// reconsider switching to csbindgen.
//
// All entry points use `LibraryImport` (source-generated P/Invoke), so
// the assembly is NativeAOT-ready out of the box.

using System.Runtime.InteropServices;

namespace Goplasmatic.Datalogic.Native;

internal static partial class NativeMethods
{
    /// <summary>
    /// The shared-library name the runtime looks up. Resolution:
    /// 1. Whatever <see cref="DllImportResolver"/> returns first (see
    ///    Engine.cs static ctor — falls back to the C ABI's cargo target
    ///    dir for local dev).
    /// 2. Otherwise the standard NuGet `runtimes/&lt;rid&gt;/native/`
    ///    layout populated at packaging time by the release workflow.
    /// </summary>
    internal const string LibraryName = "datalogic_c";

    // =============== Meta ===============

    [LibraryImport(LibraryName, EntryPoint = "datalogic_version")]
    internal static partial IntPtr datalogic_version();

    [LibraryImport(LibraryName, EntryPoint = "datalogic_string_free")]
    internal static partial void datalogic_string_free(IntPtr ptr);

    // =============== Engine ===============

    [LibraryImport(LibraryName, EntryPoint = "datalogic_engine_new")]
    internal static partial IntPtr datalogic_engine_new(int templating);

    [LibraryImport(LibraryName, EntryPoint = "datalogic_engine_free")]
    internal static partial void datalogic_engine_free(IntPtr engine);

    [LibraryImport(LibraryName, EntryPoint = "datalogic_engine_compile", StringMarshalling = StringMarshalling.Utf8)]
    internal static partial IntPtr datalogic_engine_compile(IntPtr engine, string rule_json);

    [LibraryImport(LibraryName, EntryPoint = "datalogic_engine_apply", StringMarshalling = StringMarshalling.Utf8)]
    internal static partial IntPtr datalogic_engine_apply(IntPtr engine, string rule_json, string data_json);

    [LibraryImport(LibraryName, EntryPoint = "datalogic_engine_session")]
    internal static partial IntPtr datalogic_engine_session(IntPtr engine);

    [LibraryImport(LibraryName, EntryPoint = "datalogic_engine_traced_session")]
    internal static partial IntPtr datalogic_engine_traced_session(IntPtr engine);

    // =============== Engine builder (custom operators) ===============

    [LibraryImport(LibraryName, EntryPoint = "datalogic_engine_builder_new")]
    internal static partial IntPtr datalogic_engine_builder_new();

    [LibraryImport(LibraryName, EntryPoint = "datalogic_engine_builder_free")]
    internal static partial void datalogic_engine_builder_free(IntPtr builder);

    [LibraryImport(LibraryName, EntryPoint = "datalogic_engine_builder_set_templating")]
    internal static partial void datalogic_engine_builder_set_templating(IntPtr builder, int enabled);

    [LibraryImport(LibraryName, EntryPoint = "datalogic_engine_builder_set_config_json", StringMarshalling = StringMarshalling.Utf8)]
    internal static partial int datalogic_engine_builder_set_config_json(IntPtr builder, string config_json);

    /// <summary>
    /// Callback signature for user-defined operators. Mirrors the C
    /// `datalogic_op_callback` typedef. Returns a freshly-allocated UTF-8
    /// NUL-terminated JSON string on success, or `IntPtr.Zero` on error
    /// (optionally with `*error_out` set to a freshly-allocated message).
    /// </summary>
    [UnmanagedFunctionPointer(CallingConvention.Cdecl)]
    internal unsafe delegate IntPtr OperatorCallback(IntPtr args_json, IntPtr user_data, IntPtr* error_out);

    [LibraryImport(LibraryName, EntryPoint = "datalogic_engine_builder_add_operator", StringMarshalling = StringMarshalling.Utf8)]
    internal static partial int datalogic_engine_builder_add_operator(
        IntPtr builder,
        string name,
        IntPtr callback,
        IntPtr user_data);

    [LibraryImport(LibraryName, EntryPoint = "datalogic_engine_builder_build")]
    internal static partial IntPtr datalogic_engine_builder_build(IntPtr builder);

    // =============== Rule ===============

    [LibraryImport(LibraryName, EntryPoint = "datalogic_rule_free")]
    internal static partial void datalogic_rule_free(IntPtr rule);

    [LibraryImport(LibraryName, EntryPoint = "datalogic_rule_evaluate", StringMarshalling = StringMarshalling.Utf8)]
    internal static partial IntPtr datalogic_rule_evaluate(IntPtr rule, string data_json);

    // =============== Session ===============

    [LibraryImport(LibraryName, EntryPoint = "datalogic_session_free")]
    internal static partial void datalogic_session_free(IntPtr session);

    [LibraryImport(LibraryName, EntryPoint = "datalogic_session_evaluate", StringMarshalling = StringMarshalling.Utf8)]
    internal static partial IntPtr datalogic_session_evaluate(IntPtr session, IntPtr rule, string data_json);

    [LibraryImport(LibraryName, EntryPoint = "datalogic_session_reset")]
    internal static partial void datalogic_session_reset(IntPtr session);

    [LibraryImport(LibraryName, EntryPoint = "datalogic_session_allocated_bytes")]
    internal static partial nuint datalogic_session_allocated_bytes(IntPtr session);

    // =============== Traced session ===============

    [LibraryImport(LibraryName, EntryPoint = "datalogic_traced_session_free")]
    internal static partial void datalogic_traced_session_free(IntPtr session);

    [LibraryImport(LibraryName, EntryPoint = "datalogic_traced_session_evaluate", StringMarshalling = StringMarshalling.Utf8)]
    internal static partial IntPtr datalogic_traced_session_evaluate(IntPtr session, string rule_json, string data_json);

    // =============== Last error ===============

    [LibraryImport(LibraryName, EntryPoint = "datalogic_last_error_clear")]
    internal static partial void datalogic_last_error_clear();

    [LibraryImport(LibraryName, EntryPoint = "datalogic_last_error_message")]
    internal static partial IntPtr datalogic_last_error_message();

    [LibraryImport(LibraryName, EntryPoint = "datalogic_last_error_type")]
    internal static partial IntPtr datalogic_last_error_type();

    [LibraryImport(LibraryName, EntryPoint = "datalogic_last_error_operator")]
    internal static partial IntPtr datalogic_last_error_operator();

    [LibraryImport(LibraryName, EntryPoint = "datalogic_last_error_path_json")]
    internal static partial IntPtr datalogic_last_error_path_json();

    /// <summary>
    /// Marshal a returned UTF-8 C string (callee-owned) into a managed
    /// string and free the native allocation via
    /// <see cref="datalogic_string_free"/>.
    /// </summary>
    internal static string? TakeUtf8String(IntPtr ptr)
    {
        if (ptr == IntPtr.Zero) return null;
        var s = Marshal.PtrToStringUTF8(ptr);
        datalogic_string_free(ptr);
        return s;
    }

    /// <summary>
    /// Marshal a borrowed UTF-8 C string (library-owned, never free).
    /// Used for `datalogic_last_error_*` and `datalogic_version`.
    /// </summary>
    internal static string? BorrowUtf8String(IntPtr ptr)
    {
        if (ptr == IntPtr.Zero) return null;
        return Marshal.PtrToStringUTF8(ptr);
    }
}
