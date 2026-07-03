/* SPDX-License-Identifier: Apache-2.0 */
package com.goplasmatic.datalogic;

import com.goplasmatic.datalogic.internal.DatalogicNative;

import java.lang.foreign.Arena;
import java.lang.foreign.MemorySegment;
import java.lang.foreign.ValueLayout;

/**
 * An immutable, pre-parsed JSON document — parse once, evaluate many.
 *
 * <p>A {@code DataHandle} is independent of any {@link Engine}: one
 * handle can feed rules compiled by different engines, and it is safe
 * to share across threads (evaluations only read it). It is not
 * consumed by evaluation — release it with {@link #close()} after the
 * last use.
 *
 * <pre>
 * try (DataHandle data = DataHandle.parse("{\"x\": 42}")) {
 *     rule.evaluate(data);
 *     session.evaluate(rule, data);
 * }
 * </pre>
 */
public final class DataHandle implements AutoCloseable {
    private volatile MemorySegment handle;

    private DataHandle(MemorySegment handle) {
        this.handle = handle;
    }

    /**
     * Parse a JSON document into a resident handle.
     *
     * @throws ParseException if {@code json} is not valid JSON
     */
    public static DataHandle parse(String json) {
        if (json == null) throw new NullPointerException("json");
        try (Arena arena = Arena.ofConfined()) {
            MemorySegment bytes = DatalogicNative.utf8(arena, json);
            MemorySegment out = arena.allocate(ValueLayout.ADDRESS);
            MemorySegment errSlot = arena.allocate(ValueLayout.ADDRESS);
            int status;
            try {
                status = (int) DatalogicNative.DATA_PARSE.invokeExact(
                        bytes, bytes.byteSize(), out, errSlot);
            } catch (Throwable t) {
                throw DatalogicException.propagate(t);
            }
            if (status != DatalogicNative.STATUS_OK) {
                throw DatalogicException.fromNative(status, errSlot, "data parse failed");
            }
            return new DataHandle(out.get(ValueLayout.ADDRESS, 0));
        }
    }

    MemorySegment handle() {
        MemorySegment h = handle;
        if (h == null) throw new IllegalStateException("DataHandle is closed");
        return h;
    }

    /** Bytes held by the handle's backing arena (input copy + parsed tree). */
    public long allocatedBytes() {
        try {
            return (long) DatalogicNative.DATA_ALLOCATED_BYTES.invokeExact(handle());
        } catch (Throwable t) {
            throw DatalogicException.propagate(t);
        }
    }

    @Override
    public void close() {
        MemorySegment h = handle;
        if (h != null) {
            handle = null;
            try {
                DatalogicNative.DATA_FREE.invokeExact(h);
            } catch (Throwable t) {
                throw DatalogicException.propagate(t);
            }
        }
    }
}
