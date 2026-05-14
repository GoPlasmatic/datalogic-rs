/* SPDX-License-Identifier: Apache-2.0 */
package com.goplasmatic.datalogic;

import com.goplasmatic.datalogic.internal.DatalogicNative;
import com.sun.jna.Pointer;

/**
 * Base exception for every error raised by this binding. Mirrors the
 * thread-local last-error block exposed by the C ABI
 * ({@code datalogic_last_error_*}).
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

    /** Stable error tag from the engine (e.g. "ParseError", "Thrown", "NaN"). */
    public String errorType() { return errorType; }

    /** Outermost failing operator name (e.g. "+"), or null if not operator-scoped. */
    public String operatorName() { return operatorName; }

    /** Resolved root-to-leaf error path as a JSON array, or null if not available. */
    public String pathJson() { return pathJson; }

    /**
     * Construct the right subclass by querying the C ABI's thread-local
     * last-error block.
     */
    static DatalogicException fromLastError(String fallback) {
        DatalogicNative n = DatalogicNative.INSTANCE;
        Pointer msgPtr = n.datalogic_last_error_message();
        Pointer typePtr = n.datalogic_last_error_type();
        Pointer opPtr = n.datalogic_last_error_operator();
        Pointer pathPtr = n.datalogic_last_error_path_json();
        String msg = msgPtr == null ? fallback : msgPtr.getString(0, "UTF-8");
        String type = typePtr == null ? null : typePtr.getString(0, "UTF-8");
        String op = opPtr == null ? null : opPtr.getString(0, "UTF-8");
        String path = pathPtr == null ? null : pathPtr.getString(0, "UTF-8");
        if ("ParseError".equals(type)) {
            return new ParseException(msg, type, op, path);
        }
        return new EvaluateException(msg, type, op, path);
    }
}
