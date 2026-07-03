/* SPDX-License-Identifier: Apache-2.0 */
package com.goplasmatic.datalogic;

import com.goplasmatic.datalogic.internal.DatalogicNative;

import java.lang.foreign.Arena;
import java.lang.foreign.MemorySegment;
import java.lang.foreign.ValueLayout;

/**
 * Base exception for every error raised by this binding. Carries the
 * structured fields of the C ABI's `datalogic_error` handle: the stable
 * engine tag, the outermost failing operator, and the resolved error
 * path.
 *
 * <p>Subclass mapping follows the C ABI v2 status code: parse failures
 * throw {@link ParseException}, evaluation and type-mismatch failures
 * throw {@link EvaluateException}, and invalid-argument / internal
 * failures throw this base class directly.
 */
public class DatalogicException extends RuntimeException {
    private final String errorType;
    private final String operatorName;
    private final String pathJson;

    DatalogicException(String message, String errorType, String operatorName, String pathJson) {
        super(message);
        this.errorType = errorType;
        this.operatorName = operatorName;
        this.pathJson = pathJson;
    }

    /** Stable error tag from the engine (e.g. "ParseError", "Thrown", "NaN", "TypeMismatch"). */
    public String errorType() { return errorType; }

    /** Outermost failing operator name (e.g. "+"), or null if not operator-scoped. */
    public String operatorName() { return operatorName; }

    /** Resolved root-to-leaf error path as a JSON array, or null if not available. */
    public String pathJson() { return pathJson; }

    /**
     * Rethrow helper for {@code MethodHandle.invokeExact}'s checked
     * {@link Throwable}: downcall handles to well-formed C functions do
     * not throw, so anything landing here is a JVM-level failure. Lives
     * here — outside the class holding the native downcall state — so a
     * failed {@code DatalogicNative} initialization (missing library,
     * ABI mismatch) surfaces as its original {@link Error} instead of a
     * masking {@link NoClassDefFoundError} from re-touching the failed
     * class inside a catch block.
     */
    static RuntimeException propagate(Throwable t) {
        if (t instanceof RuntimeException re) {
            return re;
        }
        if (t instanceof Error e) {
            throw e;
        }
        return new IllegalStateException("datalogic native call failed unexpectedly", t);
    }

    /**
     * Construct the right subclass from a non-OK status and the
     * `datalogic_error *` stored in {@code errSlot} by the failing call.
     * Reads message/tag/operator/path from the handle (borrowed
     * accessors), frees it, and maps the status onto the hierarchy. The
     * handle may be absent (NULL slot) — the {@code fallback} message is
     * used then.
     */
    static DatalogicException fromNative(int status, MemorySegment errSlot, String fallback) {
        String message = fallback;
        String tag = null;
        String operator = null;
        String path = null;

        MemorySegment err = errSlot.get(ValueLayout.ADDRESS, 0);
        if (err.address() != 0) {
            try (Arena scratch = Arena.ofConfined()) {
                String m = DatalogicNative.errorField(DatalogicNative.ERROR_MESSAGE, err, scratch);
                if (m != null && !m.isEmpty()) {
                    message = m;
                }
                tag = DatalogicNative.errorField(DatalogicNative.ERROR_TAG, err, scratch);
                operator = DatalogicNative.errorField(DatalogicNative.ERROR_OPERATOR, err, scratch);
                path = DatalogicNative.errorField(DatalogicNative.ERROR_PATH_JSON, err, scratch);
            } finally {
                DatalogicNative.freeError(err);
            }
        }

        return switch (status) {
            case DatalogicNative.STATUS_PARSE -> new ParseException(message, tag, operator, path);
            case DatalogicNative.STATUS_EVAL, DatalogicNative.STATUS_TYPE_MISMATCH ->
                    new EvaluateException(message, tag, operator, path);
            default -> new DatalogicException(message, tag, operator, path);
        };
    }
}
