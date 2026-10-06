/* SPDX-License-Identifier: Apache-2.0 */
package com.goplasmatic.datalogic;

import com.goplasmatic.datalogic.internal.DatalogicNative;

import java.lang.foreign.Arena;
import java.lang.foreign.MemorySegment;
import java.lang.foreign.ValueLayout;
import java.lang.ref.Reference;

/**
 * A compiled JSONLogic rule, ready to evaluate against data. Safe to
 * share across threads — each {@link #evaluate(String)} uses its own
 * short-lived arena. For tight loops, use a {@link Session} per thread.
 * A rule that is never closed is released by a
 * {@link java.lang.ref.Cleaner} once it is unreachable.
 */
public final class Rule implements AutoCloseable {
    private final NativeHandle handle;
    // Keeps the owning engine's custom-operator stubs reachable: rules
    // hold an Arc on the Rust engine and may dispatch Java operators
    // even after Engine.close().
    private final Engine owner;

    Rule(MemorySegment handle, Engine owner) {
        this.handle = new NativeHandle(this, handle, DatalogicNative.RULE_FREE, "Rule");
        this.owner = owner;
    }

    MemorySegment handle() {
        return handle.get();
    }

    /**
     * What the rule reads and calls, as JSON: {@code {reads, computed_reads,
     * reads_complete, reads_data, operators, custom_operators,
     * deterministic}}, each read path as its segments.
     */
    public String facts() {
        try (Arena arena = Arena.ofConfined()) {
            MemorySegment buf = arena.allocate(DatalogicNative.BUF_LAYOUT);
            MemorySegment errSlot = arena.allocate(ValueLayout.ADDRESS);
            int status;
            try {
                status = (int) DatalogicNative.RULE_FACTS.invokeExact(handle(), buf, errSlot);
            } catch (Throwable t) {
                throw DatalogicException.propagate(t);
            }
            if (status != DatalogicNative.STATUS_OK) {
                throw DatalogicException.fromNative(status, errSlot, "facts failed");
            }
            return DatalogicNative.takeOwnedBuf(buf);
        } finally {
            Reference.reachabilityFence(this);
        }
    }

    /** Evaluate against {@code dataJson}; returns the result JSON-string. */
    public String evaluate(String dataJson) {
        if (dataJson == null) throw new NullPointerException("dataJson");
        try (Arena arena = Arena.ofConfined()) {
            MemorySegment data = DatalogicNative.utf8(arena, dataJson);
            MemorySegment buf = arena.allocate(DatalogicNative.BUF_LAYOUT);
            MemorySegment errSlot = arena.allocate(ValueLayout.ADDRESS);
            int status;
            try {
                status = (int) DatalogicNative.RULE_EVALUATE.invokeExact(
                        handle(), data, data.byteSize(), buf, errSlot);
            } catch (Throwable t) {
                throw DatalogicException.propagate(t);
            }
            if (status != DatalogicNative.STATUS_OK) {
                throw DatalogicException.fromNative(status, errSlot, "rule evaluate failed");
            }
            return DatalogicNative.takeOwnedBuf(buf);
        } finally {
            Reference.reachabilityFence(this);
        }
    }

    /**
     * Evaluate against a pre-parsed {@link DataHandle} — skips the JSON
     * parse entirely. The handle is not consumed; reuse it across rules,
     * sessions, and threads.
     */
    public String evaluate(DataHandle data) {
        if (data == null) throw new NullPointerException("data");
        try (Arena arena = Arena.ofConfined()) {
            MemorySegment buf = arena.allocate(DatalogicNative.BUF_LAYOUT);
            MemorySegment errSlot = arena.allocate(ValueLayout.ADDRESS);
            int status;
            try {
                status = (int) DatalogicNative.RULE_EVALUATE_DATA.invokeExact(
                        handle(), data.handle(), buf, errSlot);
            } catch (Throwable t) {
                throw DatalogicException.propagate(t);
            }
            if (status != DatalogicNative.STATUS_OK) {
                throw DatalogicException.fromNative(status, errSlot, "rule evaluate failed");
            }
            return DatalogicNative.takeOwnedBuf(buf);
        } finally {
            Reference.reachabilityFence(data);
            Reference.reachabilityFence(this);
        }
    }

    /**
     * Release the native rule. Idempotent and safe to call from several
     * threads at once, but not while another thread is still evaluating
     * this rule.
     */
    @Override
    public void close() {
        handle.close();
        Reference.reachabilityFence(owner);
    }
}
