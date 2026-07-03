/* SPDX-License-Identifier: Apache-2.0 */
package com.goplasmatic.datalogic;

import com.goplasmatic.datalogic.internal.DatalogicNative;

import java.lang.foreign.Arena;
import java.lang.foreign.MemorySegment;
import java.lang.foreign.ValueLayout;
import java.lang.invoke.MethodHandle;
import java.lang.invoke.MethodHandles;
import java.lang.invoke.MethodType;
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
    private MemorySegment handle;
    private boolean consumed;
    // Strongly retain every registered bridge until the resulting
    // Engine (and everything compiled from it) goes away; the upcall
    // stubs themselves live in `stubArena`.
    private final List<Object> pinned = new ArrayList<>();
    // Automatic arena owning the upcall stubs: reclaimed by the GC only
    // once nothing (builder, engine, rule, session) references it any
    // more — never explicitly closed, so a stale function pointer can
    // never be invoked while its Java owner is still reachable.
    private Arena stubArena;

    EngineBuilder() {
        try {
            handle = (MemorySegment) DatalogicNative.BUILDER_NEW.invokeExact();
        } catch (Throwable t) {
            throw DatalogicException.propagate(t);
        }
        if (handle.address() == 0) {
            throw new DatalogicException("datalogic_engine_builder_new returned null", null, null, null);
        }
    }

    /** Toggle templating mode on the resulting engine. */
    public EngineBuilder withTemplating(boolean enabled) {
        ensureFresh();
        try {
            DatalogicNative.BUILDER_SET_TEMPLATING.invokeExact(handle, enabled ? 1 : 0);
        } catch (Throwable t) {
            throw DatalogicException.propagate(t);
        }
        return this;
    }

    /**
     * Set the engine's evaluation configuration from a JSON object
     * string, parsed by the core crate's shared config parser (the same
     * wire format every binding uses). All keys are optional; an
     * optional {@code "preset"} ({@code "default"} |
     * {@code "safe_arithmetic"} | {@code "strict"}) selects the starting
     * point and the remaining keys ({@code arithmetic_nan_handling},
     * {@code division_by_zero}, {@code loose_equality_errors},
     * {@code truthy_evaluator}, {@code numeric_coercion} as an object of
     * bools, {@code max_recursion_depth}) override individual fields on
     * top of it. Unknown keys and values are rejected (error type
     * {@code "ConfigurationError"}) so typos fail loudly instead of
     * being silently ignored. Each call replaces the builder's entire
     * evaluation config; templating and registered operators are
     * unaffected.
     *
     * @throws EvaluateException if the config JSON is malformed or
     *         contains unknown keys or values
     */
    public EngineBuilder setConfigJson(String json) {
        if (json == null) throw new NullPointerException("json");
        ensureFresh();
        try (Arena arena = Arena.ofConfined()) {
            MemorySegment config = DatalogicNative.utf8(arena, json);
            MemorySegment errSlot = arena.allocate(ValueLayout.ADDRESS);
            int status;
            try {
                status = (int) DatalogicNative.BUILDER_SET_CONFIG_JSON.invokeExact(
                        handle, config, config.byteSize(), errSlot);
            } catch (Throwable t) {
                throw DatalogicException.propagate(t);
            }
            if (status != DatalogicNative.STATUS_OK) {
                throw DatalogicException.fromNative(status, errSlot, "set_config_json failed");
            }
        }
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

        if (stubArena == null) {
            stubArena = Arena.ofAuto();
        }
        OperatorBridge bridge = new OperatorBridge(op);
        MemorySegment stub = DatalogicNative.LINKER.upcallStub(
                OperatorBridge.INVOKE.bindTo(bridge), DatalogicNative.OP_FN_DESC, stubArena);
        pinned.add(bridge);

        try (Arena arena = Arena.ofConfined()) {
            MemorySegment nameSeg = DatalogicNative.utf8(arena, name);
            MemorySegment errSlot = arena.allocate(ValueLayout.ADDRESS);
            int status;
            try {
                status = (int) DatalogicNative.BUILDER_ADD_OPERATOR.invokeExact(
                        handle, nameSeg, nameSeg.byteSize(), stub, MemorySegment.NULL, errSlot);
            } catch (Throwable t) {
                throw DatalogicException.propagate(t);
            }
            if (status != DatalogicNative.STATUS_OK) {
                throw DatalogicException.fromNative(status, errSlot, "add_operator failed");
            }
        }
        return this;
    }

    /**
     * Finalise the builder into an {@link Engine}. The builder is
     * consumed; subsequent calls throw {@link IllegalStateException}.
     */
    public Engine build() {
        ensureFresh();
        MemorySegment enginePtr;
        try {
            enginePtr = (MemorySegment) DatalogicNative.BUILDER_BUILD.invokeExact(handle);
            DatalogicNative.BUILDER_FREE.invokeExact(handle);
        } catch (Throwable t) {
            throw DatalogicException.propagate(t);
        }
        handle = null;
        consumed = true;
        if (enginePtr.address() == 0) {
            throw new DatalogicException("builder build failed", null, null, null);
        }
        return new Engine(enginePtr, pinned, stubArena);
    }

    private void ensureFresh() {
        if (consumed) throw new IllegalStateException("EngineBuilder has already been built");
        if (handle == null) throw new IllegalStateException("EngineBuilder is invalid");
    }

    /**
     * Java side of `datalogic_op_fn`: decodes the borrowed args JSON,
     * invokes the user's {@link CustomOperator}, and writes the outcome
     * through `datalogic_op_result_set_json` / `_set_error` (both copy
     * immediately, so per-call confined arenas are safe). Returns 0 on
     * success, 1 on failure. No Throwable ever crosses the upcall — an
     * exception unwinding into native code would tear the VM down.
     */
    private static final class OperatorBridge {
        static final MethodHandle INVOKE;

        static {
            try {
                INVOKE = MethodHandles.lookup().findVirtual(
                        OperatorBridge.class,
                        "invoke",
                        MethodType.methodType(int.class,
                                MemorySegment.class, long.class, MemorySegment.class, MemorySegment.class));
            } catch (ReflectiveOperationException e) {
                throw new ExceptionInInitializerError(e);
            }
        }

        private final CustomOperator op;

        OperatorBridge(CustomOperator op) { this.op = op; }

        @SuppressWarnings("unused") // invoked reflectively through INVOKE
        int invoke(MemorySegment argsJson, long argsLen, MemorySegment userData, MemorySegment out) {
            try {
                String args = argsLen == 0 ? "[]" : DatalogicNative.readUtf8(argsJson, argsLen);
                String result = op.invoke(args);
                if (result == null) {
                    return fail(out, "custom operator returned null result");
                }
                try (Arena arena = Arena.ofConfined()) {
                    MemorySegment json = DatalogicNative.utf8(arena, result);
                    DatalogicNative.OP_RESULT_SET_JSON.invokeExact(out, json, json.byteSize());
                }
                return 0;
            } catch (Throwable t) {
                String message = t.getMessage() == null ? t.getClass().getSimpleName() : t.getMessage();
                return fail(out, message);
            }
        }

        private static int fail(MemorySegment out, String message) {
            try (Arena arena = Arena.ofConfined()) {
                MemorySegment msg = DatalogicNative.utf8(arena, message);
                DatalogicNative.OP_RESULT_SET_ERROR.invokeExact(out, msg, msg.byteSize());
            } catch (Throwable ignored) {
                // a bare non-zero return still yields a generic error
                // naming the operator — never propagate across the upcall
            }
            return 1;
        }
    }
}
