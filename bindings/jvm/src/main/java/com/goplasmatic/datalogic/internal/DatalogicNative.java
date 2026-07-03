/*
 * SPDX-License-Identifier: Apache-2.0
 *
 * java.lang.foreign (FFM) bindings for libdatalogic_c, C ABI v2.
 * Mirrors `bindings/c/include/datalogic.h` 1:1 — keep entry points and
 * descriptors in sync when the C ABI changes.
 *
 * v2 contract highlights (see the header for the full text):
 *   - Byte inputs cross as (pointer, length) UTF-8 — never
 *     NUL-terminated. Java strings are encoded with an explicit
 *     StandardCharsets.UTF_8 into a per-call confined arena.
 *   - Fallible calls return a status int and take a trailing
 *     `datalogic_error **err` out-param; non-OK reads the error handle
 *     (message/tag/operator/path) and frees it.
 *   - Session results are borrowed (ptr,len) — copied to a String
 *     immediately. One-shot results are owned `datalogic_buf` structs
 *     released via `datalogic_buf_free` (passed BY VALUE).
 *
 * Every downcall handle is created eagerly at class initialization,
 * which doubles as the load-time symbol check; a missing symbol fails
 * here, loudly, instead of at first use. `datalogic_abi_version()` is
 * asserted == 2 in the same breath.
 */

package com.goplasmatic.datalogic.internal;

import java.lang.foreign.Arena;
import java.lang.foreign.FunctionDescriptor;
import java.lang.foreign.Linker;
import java.lang.foreign.MemoryLayout;
import java.lang.foreign.MemorySegment;
import java.lang.foreign.SegmentAllocator;
import java.lang.foreign.StructLayout;
import java.lang.foreign.SymbolLookup;
import java.lang.foreign.ValueLayout;
import java.lang.invoke.MethodHandle;
import java.nio.charset.StandardCharsets;

/**
 * FFM downcall handles + marshalling helpers for libdatalogic_c (ABI
 * v2). Internal — not part of the binding's supported API.
 */
public final class DatalogicNative {

    private DatalogicNative() {}

    /** The C ABI generation this binding is compiled against. */
    public static final int EXPECTED_ABI_VERSION = 2;

    // =============== status codes (datalogic_status) ===============

    public static final int STATUS_OK = 0;
    public static final int STATUS_INVALID_ARG = 1;
    public static final int STATUS_PARSE = 2;
    public static final int STATUS_EVAL = 3;
    public static final int STATUS_TYPE_MISMATCH = 4;
    public static final int STATUS_INTERNAL = 5;

    // =============== layouts ===============

    /** All six supported platforms are LP64/LLP64 with 64-bit size_t. */
    static final ValueLayout.OfLong SIZE_T = ValueLayout.JAVA_LONG;

    /** `datalogic_buf { uint8_t *ptr; size_t len; size_t cap; }` — owned, by-value free. */
    public static final StructLayout BUF_LAYOUT = MemoryLayout.structLayout(
            ValueLayout.ADDRESS.withName("ptr"),
            SIZE_T.withName("len"),
            SIZE_T.withName("cap"));

    /** `datalogic_slice { const uint8_t *ptr; size_t len; }` — borrowed batch results. */
    public static final StructLayout SLICE_LAYOUT = MemoryLayout.structLayout(
            ValueLayout.ADDRESS.withName("ptr"),
            SIZE_T.withName("len"));

    /** `int32_t (*datalogic_op_fn)(const uint8_t*, size_t, void*, datalogic_op_result*)`. */
    public static final FunctionDescriptor OP_FN_DESC = FunctionDescriptor.of(
            ValueLayout.JAVA_INT,
            ValueLayout.ADDRESS, SIZE_T, ValueLayout.ADDRESS, ValueLayout.ADDRESS);

    // =============== linker plumbing ===============

    public static final Linker LINKER = Linker.nativeLinker();
    private static final SymbolLookup LOOKUP = NativeLibrary.load();

    private static MethodHandle dh(String symbol, FunctionDescriptor descriptor) {
        MemorySegment address = LOOKUP.find(symbol).orElseThrow(() -> new UnsatisfiedLinkError(
                "datalogic native library is missing symbol '" + symbol
                        + "' — it predates C ABI v" + EXPECTED_ABI_VERSION
                        + "; rebuild bindings/c (`cargo build --release`) or upgrade the packaged library"));
        return LINKER.downcallHandle(address, descriptor);
    }

    // =============== meta ===============

    public static final MethodHandle ABI_VERSION =
            dh("datalogic_abi_version", FunctionDescriptor.of(ValueLayout.JAVA_INT));
    public static final MethodHandle VERSION =
            dh("datalogic_version", FunctionDescriptor.of(ValueLayout.ADDRESS));
    /** Takes the `datalogic_buf` struct BY VALUE. */
    public static final MethodHandle BUF_FREE =
            dh("datalogic_buf_free", FunctionDescriptor.ofVoid(BUF_LAYOUT));

    // =============== engine ===============

    public static final MethodHandle ENGINE_NEW = dh("datalogic_engine_new",
            FunctionDescriptor.of(ValueLayout.ADDRESS, ValueLayout.JAVA_INT));
    public static final MethodHandle ENGINE_FREE = dh("datalogic_engine_free",
            FunctionDescriptor.ofVoid(ValueLayout.ADDRESS));
    public static final MethodHandle ENGINE_COMPILE = dh("datalogic_engine_compile",
            FunctionDescriptor.of(ValueLayout.JAVA_INT,
                    ValueLayout.ADDRESS, ValueLayout.ADDRESS, SIZE_T,
                    ValueLayout.ADDRESS, ValueLayout.ADDRESS));
    public static final MethodHandle ENGINE_APPLY = dh("datalogic_engine_apply",
            FunctionDescriptor.of(ValueLayout.JAVA_INT,
                    ValueLayout.ADDRESS, ValueLayout.ADDRESS, SIZE_T,
                    ValueLayout.ADDRESS, SIZE_T, ValueLayout.ADDRESS, ValueLayout.ADDRESS));
    public static final MethodHandle ENGINE_SESSION = dh("datalogic_engine_session",
            FunctionDescriptor.of(ValueLayout.ADDRESS, ValueLayout.ADDRESS));
    public static final MethodHandle ENGINE_TRACED_SESSION = dh("datalogic_engine_traced_session",
            FunctionDescriptor.of(ValueLayout.ADDRESS, ValueLayout.ADDRESS));

    // =============== engine builder ===============

    public static final MethodHandle BUILDER_NEW = dh("datalogic_engine_builder_new",
            FunctionDescriptor.of(ValueLayout.ADDRESS));
    public static final MethodHandle BUILDER_FREE = dh("datalogic_engine_builder_free",
            FunctionDescriptor.ofVoid(ValueLayout.ADDRESS));
    public static final MethodHandle BUILDER_SET_TEMPLATING = dh("datalogic_engine_builder_set_templating",
            FunctionDescriptor.ofVoid(ValueLayout.ADDRESS, ValueLayout.JAVA_INT));
    public static final MethodHandle BUILDER_SET_CONFIG_JSON = dh("datalogic_engine_builder_set_config_json",
            FunctionDescriptor.of(ValueLayout.JAVA_INT,
                    ValueLayout.ADDRESS, ValueLayout.ADDRESS, SIZE_T, ValueLayout.ADDRESS));
    public static final MethodHandle BUILDER_ADD_OPERATOR = dh("datalogic_engine_builder_add_operator",
            FunctionDescriptor.of(ValueLayout.JAVA_INT,
                    ValueLayout.ADDRESS, ValueLayout.ADDRESS, SIZE_T,
                    ValueLayout.ADDRESS, ValueLayout.ADDRESS, ValueLayout.ADDRESS));
    public static final MethodHandle BUILDER_BUILD = dh("datalogic_engine_builder_build",
            FunctionDescriptor.of(ValueLayout.ADDRESS, ValueLayout.ADDRESS));

    // =============== custom-operator result setters ===============

    public static final MethodHandle OP_RESULT_SET_JSON = dh("datalogic_op_result_set_json",
            FunctionDescriptor.ofVoid(ValueLayout.ADDRESS, ValueLayout.ADDRESS, SIZE_T));
    public static final MethodHandle OP_RESULT_SET_ERROR = dh("datalogic_op_result_set_error",
            FunctionDescriptor.ofVoid(ValueLayout.ADDRESS, ValueLayout.ADDRESS, SIZE_T));

    // =============== data handles ===============

    public static final MethodHandle DATA_PARSE = dh("datalogic_data_parse",
            FunctionDescriptor.of(ValueLayout.JAVA_INT,
                    ValueLayout.ADDRESS, SIZE_T, ValueLayout.ADDRESS, ValueLayout.ADDRESS));
    public static final MethodHandle DATA_FREE = dh("datalogic_data_free",
            FunctionDescriptor.ofVoid(ValueLayout.ADDRESS));
    public static final MethodHandle DATA_ALLOCATED_BYTES = dh("datalogic_data_allocated_bytes",
            FunctionDescriptor.of(SIZE_T, ValueLayout.ADDRESS));

    // =============== rule ===============

    public static final MethodHandle RULE_FREE = dh("datalogic_rule_free",
            FunctionDescriptor.ofVoid(ValueLayout.ADDRESS));
    public static final MethodHandle RULE_EVALUATE = dh("datalogic_rule_evaluate",
            FunctionDescriptor.of(ValueLayout.JAVA_INT,
                    ValueLayout.ADDRESS, ValueLayout.ADDRESS, SIZE_T,
                    ValueLayout.ADDRESS, ValueLayout.ADDRESS));
    public static final MethodHandle RULE_EVALUATE_DATA = dh("datalogic_rule_evaluate_data",
            FunctionDescriptor.of(ValueLayout.JAVA_INT,
                    ValueLayout.ADDRESS, ValueLayout.ADDRESS,
                    ValueLayout.ADDRESS, ValueLayout.ADDRESS));

    // =============== session ===============

    public static final MethodHandle SESSION_FREE = dh("datalogic_session_free",
            FunctionDescriptor.ofVoid(ValueLayout.ADDRESS));
    public static final MethodHandle SESSION_RESET = dh("datalogic_session_reset",
            FunctionDescriptor.ofVoid(ValueLayout.ADDRESS));
    public static final MethodHandle SESSION_ALLOCATED_BYTES = dh("datalogic_session_allocated_bytes",
            FunctionDescriptor.of(SIZE_T, ValueLayout.ADDRESS));
    public static final MethodHandle SESSION_EVALUATE = dh("datalogic_session_evaluate",
            FunctionDescriptor.of(ValueLayout.JAVA_INT,
                    ValueLayout.ADDRESS, ValueLayout.ADDRESS, ValueLayout.ADDRESS, SIZE_T,
                    ValueLayout.ADDRESS, ValueLayout.ADDRESS, ValueLayout.ADDRESS));
    public static final MethodHandle SESSION_EVALUATE_DATA = dh("datalogic_session_evaluate_data",
            FunctionDescriptor.of(ValueLayout.JAVA_INT,
                    ValueLayout.ADDRESS, ValueLayout.ADDRESS, ValueLayout.ADDRESS,
                    ValueLayout.ADDRESS, ValueLayout.ADDRESS, ValueLayout.ADDRESS));
    private static final FunctionDescriptor TYPED_EVAL_DESC = FunctionDescriptor.of(
            ValueLayout.JAVA_INT,
            ValueLayout.ADDRESS, ValueLayout.ADDRESS, ValueLayout.ADDRESS,
            ValueLayout.ADDRESS, ValueLayout.ADDRESS);
    public static final MethodHandle SESSION_EVALUATE_BOOL =
            dh("datalogic_session_evaluate_bool", TYPED_EVAL_DESC);
    public static final MethodHandle SESSION_EVALUATE_I64 =
            dh("datalogic_session_evaluate_i64", TYPED_EVAL_DESC);
    public static final MethodHandle SESSION_EVALUATE_F64 =
            dh("datalogic_session_evaluate_f64", TYPED_EVAL_DESC);
    public static final MethodHandle SESSION_EVALUATE_TRUTHY =
            dh("datalogic_session_evaluate_truthy", TYPED_EVAL_DESC);
    public static final MethodHandle SESSION_EVALUATE_BATCH = dh("datalogic_session_evaluate_batch",
            FunctionDescriptor.of(ValueLayout.JAVA_INT,
                    ValueLayout.ADDRESS, ValueLayout.ADDRESS, ValueLayout.ADDRESS, SIZE_T,
                    ValueLayout.ADDRESS, ValueLayout.ADDRESS, ValueLayout.ADDRESS));
    public static final MethodHandle SESSION_EVALUATE_MANY = dh("datalogic_session_evaluate_many",
            FunctionDescriptor.of(ValueLayout.JAVA_INT,
                    ValueLayout.ADDRESS, ValueLayout.ADDRESS, SIZE_T, ValueLayout.ADDRESS,
                    ValueLayout.ADDRESS, ValueLayout.ADDRESS, ValueLayout.ADDRESS));

    // =============== traced session ===============

    public static final MethodHandle TRACED_SESSION_FREE = dh("datalogic_traced_session_free",
            FunctionDescriptor.ofVoid(ValueLayout.ADDRESS));
    public static final MethodHandle TRACED_SESSION_EVALUATE = dh("datalogic_traced_session_evaluate",
            FunctionDescriptor.of(ValueLayout.JAVA_INT,
                    ValueLayout.ADDRESS, ValueLayout.ADDRESS, SIZE_T,
                    ValueLayout.ADDRESS, SIZE_T, ValueLayout.ADDRESS, ValueLayout.ADDRESS));

    // =============== error handles ===============

    public static final MethodHandle ERROR_FREE = dh("datalogic_error_free",
            FunctionDescriptor.ofVoid(ValueLayout.ADDRESS));
    public static final MethodHandle ERROR_STATUS = dh("datalogic_error_status",
            FunctionDescriptor.of(ValueLayout.JAVA_INT, ValueLayout.ADDRESS));
    private static final FunctionDescriptor ERROR_ACCESSOR_DESC = FunctionDescriptor.of(
            ValueLayout.ADDRESS, ValueLayout.ADDRESS, ValueLayout.ADDRESS);
    public static final MethodHandle ERROR_MESSAGE = dh("datalogic_error_message", ERROR_ACCESSOR_DESC);
    public static final MethodHandle ERROR_TAG = dh("datalogic_error_tag", ERROR_ACCESSOR_DESC);
    public static final MethodHandle ERROR_OPERATOR = dh("datalogic_error_operator", ERROR_ACCESSOR_DESC);
    public static final MethodHandle ERROR_PATH_JSON = dh("datalogic_error_path_json", ERROR_ACCESSOR_DESC);

    // The ABI assert runs after the handles above exist (each dh() call
    // already proved its symbol resolves; this proves the *semantics*
    // match too — v1 libraries never export datalogic_abi_version, so
    // they fail at the dh() stage with the missing-symbol message).
    static {
        int abi = abiVersion();
        if (abi != EXPECTED_ABI_VERSION) {
            throw new UnsatisfiedLinkError(
                    "datalogic native library reports C ABI version " + abi + ", but this binding requires "
                            + EXPECTED_ABI_VERSION + ". You are mixing a stale libdatalogic_c with a newer JAR "
                            + "(or vice versa) — rebuild bindings/c (`cargo build --release`) or align the "
                            + "JAR and native library versions.");
        }
    }

    /** Runtime C ABI version of the loaded library. */
    public static int abiVersion() {
        try {
            return (int) ABI_VERSION.invokeExact();
        } catch (Throwable t) {
            throw propagate(t);
        }
    }

    // =============== marshalling helpers ===============

    /**
     * Encode a Java string as UTF-8 bytes in {@code allocator} — the v2
     * input convention: exact (ptr,len), no NUL terminator, and an
     * explicit charset (never the platform default).
     */
    public static MemorySegment utf8(SegmentAllocator allocator, String s) {
        return allocator.allocateFrom(ValueLayout.JAVA_BYTE, s.getBytes(StandardCharsets.UTF_8));
    }

    /**
     * Copy {@code len} bytes at {@code ptr} out of native memory as a
     * UTF-8 string. Used for borrowed results — the copy must happen
     * before the next call touching the owning session/error.
     */
    public static String readUtf8(MemorySegment ptr, long len) {
        if (len == 0) {
            return "";
        }
        byte[] bytes = ptr.reinterpret(len).toArray(ValueLayout.JAVA_BYTE);
        return new String(bytes, StandardCharsets.UTF_8);
    }

    /**
     * Read an owned `datalogic_buf` (filled by a one-shot entry point
     * through an out-pointer) into a String, then release it via
     * `datalogic_buf_free(buf)` — struct passed by value.
     */
    public static String takeOwnedBuf(MemorySegment bufStruct) {
        MemorySegment ptr = bufStruct.get(ValueLayout.ADDRESS, 0);
        long len = bufStruct.get(SIZE_T, ValueLayout.ADDRESS.byteSize());
        String result = ptr.address() == 0 ? "" : readUtf8(ptr, len);
        try {
            BUF_FREE.invokeExact(bufStruct);
        } catch (Throwable t) {
            throw propagate(t);
        }
        return result;
    }

    /**
     * Read a borrowed error-accessor field: `const uint8_t *fn(err,
     * size_t *len_out)`. Returns {@code null} for absent fields.
     */
    public static String errorField(MethodHandle accessor, MemorySegment err, SegmentAllocator scratch) {
        try {
            MemorySegment lenOut = scratch.allocate(SIZE_T);
            MemorySegment ptr = (MemorySegment) accessor.invokeExact(err, lenOut);
            if (ptr.address() == 0) {
                return null;
            }
            return readUtf8(ptr, lenOut.get(SIZE_T, 0));
        } catch (Throwable t) {
            throw propagate(t);
        }
    }

    /** Release an error handle; never throws. */
    public static void freeError(MemorySegment err) {
        try {
            ERROR_FREE.invokeExact(err);
        } catch (Throwable ignored) {
            // freeing is best-effort; the handle is small
        }
    }

    /**
     * Rethrow helper for {@code MethodHandle.invokeExact}'s checked
     * {@link Throwable}: downcall handles to well-formed C functions do
     * not throw, so anything landing here is a JVM-level failure.
     */
    public static RuntimeException propagate(Throwable t) {
        if (t instanceof RuntimeException re) {
            return re;
        }
        if (t instanceof Error e) {
            throw e;
        }
        return new IllegalStateException("datalogic native call failed unexpectedly", t);
    }
}
