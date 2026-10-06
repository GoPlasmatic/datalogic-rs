/* SPDX-License-Identifier: Apache-2.0 */
package com.goplasmatic.datalogic;

import com.fasterxml.jackson.databind.JsonNode;
import com.fasterxml.jackson.databind.ObjectMapper;
import com.goplasmatic.datalogic.internal.DatalogicNative;

import java.io.IOException;
import java.lang.foreign.Arena;
import java.lang.foreign.MemorySegment;
import java.lang.foreign.ValueLayout;
import java.lang.ref.Reference;

/**
 * Trace-enabled handle over an {@link Engine}. Every
 * {@link #evaluate(String, String)} call returns a {@link TracedRun}
 * carrying the result alongside execution-step and expression-tree
 * metadata. Thread-safe — share freely.
 */
public final class TracedSession implements AutoCloseable {
    private static final ObjectMapper MAPPER = new ObjectMapper();

    private final NativeHandle handle;
    // Keeps the owning engine's custom-operator stubs reachable while a
    // traced evaluation (which may dispatch into Java) is in flight.
    private final Engine owner;

    TracedSession(MemorySegment handle, Engine owner) {
        this.handle = new NativeHandle(this, handle, DatalogicNative.TRACED_SESSION_FREE, "TracedSession");
        this.owner = owner;
    }

    private MemorySegment handle() {
        return handle.get();
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
        return evaluate(ruleJson, dataJson, CompileMode.ENGINE);
    }

    /**
     * {@link #evaluate(String, String)} with the rule compiled in
     * {@code mode}, as {@link Engine#compileMode} compiles it, so a rule
     * compiled as a template is traced as one.
     */
    public TracedRun evaluate(String ruleJson, String dataJson, CompileMode mode) {
        if (ruleJson == null) throw new NullPointerException("ruleJson");
        if (dataJson == null) throw new NullPointerException("dataJson");
        if (mode == null) throw new NullPointerException("mode");
        String payload;
        try (Arena arena = Arena.ofConfined()) {
            MemorySegment rule = DatalogicNative.utf8(arena, ruleJson);
            MemorySegment data = DatalogicNative.utf8(arena, dataJson);
            MemorySegment buf = arena.allocate(DatalogicNative.BUF_LAYOUT);
            MemorySegment errSlot = arena.allocate(ValueLayout.ADDRESS);
            int status;
            try {
                status = (int) DatalogicNative.TRACED_SESSION_EVALUATE_MODE.invokeExact(
                        handle(), rule, rule.byteSize(), data, data.byteSize(), mode.code(), buf, errSlot);
            } catch (Throwable t) {
                throw DatalogicException.propagate(t);
            }
            if (status != DatalogicNative.STATUS_OK) {
                throw DatalogicException.fromNative(status, errSlot, "traced session evaluate failed");
            }
            payload = DatalogicNative.takeOwnedBuf(buf);
        } finally {
            Reference.reachabilityFence(this);
        }
        try {
            JsonNode node = MAPPER.readTree(payload);
            return new TracedRun(
                    node.get("result"),
                    node.get("expression_tree"),
                    node.get("steps"),
                    node.has("error") ? node.get("error").asText(null) : null,
                    node.get("structured_error"),
                    node.get("pointers")
            );
        } catch (IOException e) {
            throw new EvaluateException(
                    "traced session returned malformed payload: " + e.getMessage(),
                    null, null, null);
        }
    }

    /**
     * Release the native traced session. Idempotent and safe to call from
     * several threads at once, but not while another thread is still
     * evaluating through it. A traced session that is never closed is
     * released by a {@link java.lang.ref.Cleaner} once it is unreachable.
     */
    @Override
    public void close() {
        handle.close();
        Reference.reachabilityFence(owner);
    }
}
