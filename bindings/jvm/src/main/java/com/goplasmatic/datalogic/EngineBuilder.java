/* SPDX-License-Identifier: Apache-2.0 */
package com.goplasmatic.datalogic;

import com.goplasmatic.datalogic.internal.DatalogicNative;
import com.sun.jna.Native;
import com.sun.jna.Pointer;
import com.sun.jna.ptr.PointerByReference;

import java.nio.charset.StandardCharsets;
import java.util.ArrayList;
import java.util.List;

/**
 * Builder for engines with custom JSONLogic operators implemented in
 * Java. Mirrors the cross-binding contract: registering a name that
 * collides with a built-in ({@code +}, {@code if}, {@code var}, …)
 * silently dispatches to the built-in at evaluation time — built-ins
 * always win.
 */
public final class EngineBuilder {
    private Pointer handle;
    private boolean consumed;
    // Strongly retain every registered callback until the resulting
    // Engine is closed; otherwise JNA may GC them while the engine
    // still holds the function pointer.
    private final List<DatalogicNative.OperatorCallback> pinned = new ArrayList<>();

    EngineBuilder() {
        handle = DatalogicNative.INSTANCE.datalogic_engine_builder_new();
        if (handle == null) {
            throw DatalogicException.fromLastError("builder_new failed");
        }
    }

    /** Toggle templating mode on the resulting engine. */
    public EngineBuilder withTemplating(boolean enabled) {
        ensureFresh();
        DatalogicNative.INSTANCE.datalogic_engine_builder_set_templating(handle, enabled ? 1 : 0);
        return this;
    }

    /**
     * Register a custom JSONLogic operator under {@code name}. The
     * {@link CustomOperator} contract takes a JSON-array string of
     * pre-evaluated arguments and returns a JSON-value string.
     */
    public EngineBuilder addOperator(String name, CustomOperator op) {
        if (name == null || name.isEmpty()) throw new IllegalArgumentException("name");
        if (op == null) throw new NullPointerException("op");
        ensureFresh();

        DatalogicNative.OperatorCallback cb = (argsJsonPtr, userData, errorOut) -> {
            try {
                String args = argsJsonPtr == null ? "[]" : argsJsonPtr.getString(0, "UTF-8");
                String result = op.invoke(args);
                if (result == null) {
                    setError(errorOut, "custom operator returned null result");
                    return null;
                }
                return allocLibcUtf8(result);
            } catch (Throwable t) {
                setError(errorOut, t.getMessage() == null ? t.getClass().getSimpleName() : t.getMessage());
                return null;
            }
        };
        pinned.add(cb);
        int rc = DatalogicNative.INSTANCE.datalogic_engine_builder_add_operator(handle, name, cb, null);
        if (rc != 0) {
            throw DatalogicException.fromLastError("add_operator failed");
        }
        return this;
    }

    /**
     * Finalise the builder into an {@link Engine}. The builder is
     * consumed; subsequent calls throw {@link IllegalStateException}.
     */
    public Engine build() {
        ensureFresh();
        Pointer enginePtr = DatalogicNative.INSTANCE.datalogic_engine_builder_build(handle);
        DatalogicNative.INSTANCE.datalogic_engine_builder_free(handle);
        handle = null;
        consumed = true;
        if (enginePtr == null) {
            throw DatalogicException.fromLastError("builder build failed");
        }
        return new Engine(enginePtr, pinned);
    }

    private void ensureFresh() {
        if (consumed) throw new IllegalStateException("EngineBuilder has already been built");
        if (handle == null) throw new IllegalStateException("EngineBuilder is invalid");
    }

    /**
     * Allocate a UTF-8 NUL-terminated buffer using the C runtime's
     * malloc — Rust calls libc {@code free()} on this pointer, so we
     * have to use the matching allocator. JNA's {@link Native#malloc} is
     * a thin wrapper around libc malloc on every supported platform.
     */
    private static Pointer allocLibcUtf8(String s) {
        byte[] bytes = s.getBytes(StandardCharsets.UTF_8);
        long ptr = Native.malloc(bytes.length + 1);
        if (ptr == 0) throw new OutOfMemoryError("Native.malloc failed");
        Pointer p = new Pointer(ptr);
        p.write(0, bytes, 0, bytes.length);
        p.setByte(bytes.length, (byte) 0);
        return p;
    }

    private static void setError(PointerByReference errorOut, String msg) {
        if (errorOut != null && msg != null) {
            errorOut.setValue(allocLibcUtf8(msg));
        }
    }
}
