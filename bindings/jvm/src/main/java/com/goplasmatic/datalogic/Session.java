/* SPDX-License-Identifier: Apache-2.0 */
package com.goplasmatic.datalogic;

import com.goplasmatic.datalogic.internal.DatalogicNative;
import com.sun.jna.Pointer;

/**
 * Hot-loop session bound to a single {@link Engine}. Reuses one
 * {@code bumpalo::Bump} across evaluations and resets it at the start of
 * every call so peak memory stays bounded. NOT thread-safe — open one
 * per thread.
 */
public final class Session implements AutoCloseable {
    private Pointer handle;

    Session(Pointer handle) { this.handle = handle; }

    private Pointer handle() {
        if (handle == null) throw new IllegalStateException("Session is closed");
        return handle;
    }

    /**
     * Evaluate {@code rule} against {@code dataJson} using this session's
     * reusable arena.
     */
    public String evaluate(Rule rule, String dataJson) {
        if (rule == null) throw new NullPointerException("rule");
        if (dataJson == null) throw new NullPointerException("dataJson");
        Pointer ptr = DatalogicNative.INSTANCE.datalogic_session_evaluate(handle(), rule.handle(), dataJson);
        if (ptr == null) throw DatalogicException.fromLastError("session evaluate failed");
        return Engine.takeOwnedString(ptr);
    }

    /**
     * Manually reset the session's arena. Optional — every
     * {@link #evaluate} already resets at the start of the call.
     */
    public void reset() {
        DatalogicNative.INSTANCE.datalogic_session_reset(handle());
    }

    /**
     * Bytes currently held by the session's arena (sum across all
     * chunks).
     */
    public long allocatedBytes() {
        return DatalogicNative.INSTANCE.datalogic_session_allocated_bytes(handle());
    }

    @Override
    public void close() {
        if (handle != null) {
            DatalogicNative.INSTANCE.datalogic_session_free(handle);
            handle = null;
        }
    }
}
