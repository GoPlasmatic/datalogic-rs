/* SPDX-License-Identifier: Apache-2.0 */
package com.goplasmatic.datalogic;

import com.goplasmatic.datalogic.internal.DatalogicNative;
import com.sun.jna.Pointer;

/**
 * A compiled JSONLogic rule, ready to evaluate against data. Safe to
 * share across threads — each {@link #evaluate(String)} uses its own
 * short-lived arena. For tight loops, use a {@link Session} per thread.
 */
public final class Rule implements AutoCloseable {
    private Pointer handle;

    Rule(Pointer handle) { this.handle = handle; }

    Pointer handle() {
        if (handle == null) throw new IllegalStateException("Rule is closed");
        return handle;
    }

    /** Evaluate against {@code dataJson}; returns the result JSON-string. */
    public String evaluate(String dataJson) {
        if (dataJson == null) throw new NullPointerException("dataJson");
        Pointer ptr = DatalogicNative.INSTANCE.datalogic_rule_evaluate(handle(), dataJson);
        if (ptr == null) throw DatalogicException.fromLastError("rule evaluate failed");
        return Engine.takeOwnedString(ptr);
    }

    @Override
    public void close() {
        if (handle != null) {
            DatalogicNative.INSTANCE.datalogic_rule_free(handle);
            handle = null;
        }
    }
}
