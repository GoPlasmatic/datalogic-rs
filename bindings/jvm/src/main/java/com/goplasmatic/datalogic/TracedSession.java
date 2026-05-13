/* SPDX-License-Identifier: Apache-2.0 */
package com.goplasmatic.datalogic;

import com.fasterxml.jackson.databind.JsonNode;
import com.fasterxml.jackson.databind.ObjectMapper;
import com.goplasmatic.datalogic.internal.DatalogicNative;
import com.sun.jna.Pointer;

import java.io.IOException;

/**
 * Trace-enabled handle over an {@link Engine}. Every
 * {@link #evaluate(String, String)} call returns a {@link TracedRun}
 * carrying the result alongside execution-step and expression-tree
 * metadata. Thread-safe — share freely.
 */
public final class TracedSession implements AutoCloseable {
    private static final ObjectMapper MAPPER = new ObjectMapper();

    private Pointer handle;

    TracedSession(Pointer handle) { this.handle = handle; }

    private Pointer handle() {
        if (handle == null) throw new IllegalStateException("TracedSession is closed");
        return handle;
    }

    /**
     * One-shot traced evaluation: compile {@code ruleJson} internally
     * with the optimizer disabled, evaluate against {@code dataJson}, and
     * return the result + trace. Engine errors surface inside the
     * returned {@link TracedRun} (see {@link TracedRun#error()}) rather
     * than as a thrown exception — the trace data is always returned
     * alongside, even on failure.
     */
    public TracedRun evaluate(String ruleJson, String dataJson) {
        if (ruleJson == null) throw new NullPointerException("ruleJson");
        if (dataJson == null) throw new NullPointerException("dataJson");
        Pointer ptr = DatalogicNative.INSTANCE.datalogic_traced_session_evaluate(handle(), ruleJson, dataJson);
        if (ptr == null) throw DatalogicException.fromLastError("traced session evaluate failed");
        String payload = Engine.takeOwnedString(ptr);
        try {
            JsonNode node = MAPPER.readTree(payload);
            return new TracedRun(
                    node.get("result"),
                    node.get("expression_tree"),
                    node.get("steps"),
                    node.has("error") ? node.get("error").asText(null) : null,
                    node.get("structured_error")
            );
        } catch (IOException e) {
            throw new EvaluateException(
                    "traced session returned malformed payload: " + e.getMessage(),
                    null, null, null);
        }
    }

    @Override
    public void close() {
        if (handle != null) {
            DatalogicNative.INSTANCE.datalogic_traced_session_free(handle);
            handle = null;
        }
    }
}
