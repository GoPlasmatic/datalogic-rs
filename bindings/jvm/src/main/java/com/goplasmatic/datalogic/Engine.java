/* SPDX-License-Identifier: Apache-2.0 */
package com.goplasmatic.datalogic;

import com.goplasmatic.datalogic.internal.DatalogicNative;

import java.lang.foreign.Arena;
import java.lang.foreign.MemorySegment;
import java.lang.foreign.ValueLayout;
import java.lang.ref.Reference;
import java.util.ArrayList;
import java.util.List;

/**
 * A JSONLogic compile/evaluate engine. Wraps a shared
 * {@code Arc<datalogic_rs::Engine>} on the Rust side and is safe to
 * share across threads. {@link #close()} releases the native handle; an
 * engine that is never closed is released by a {@link java.lang.ref.Cleaner}
 * once it is unreachable. {@code close()} is idempotent and safe to call
 * from several threads at once, but not while another thread is still
 * using the engine.
 *
 * <pre>
 * try (Engine engine = new Engine()) {
 *     String result = engine.apply("{\"+\":[1,2]}", "{}");  // "3"
 * }
 * </pre>
 */
public class Engine implements AutoCloseable {
    private final NativeHandle handle;
    // Custom-operator upcall stubs live in an automatic arena; keep the
    // arena (and the bound bridges) strongly reachable for as long as
    // the engine — and anything compiled from it — is, so the native
    // side never dispatches into a reclaimed stub.
    final List<Object> retainedCallbacks = new ArrayList<>();
    private final Arena callbackArena;

    /** Construct an engine with default (non-templating) configuration. */
    public Engine() {
        this(false);
    }

    /**
     * Construct an engine, optionally enabling templating mode where
     * multi-key objects in compiled rules become output-shaping templates.
     */
    public Engine(boolean templating) {
        this(newEngine(templating), null, null);
    }

    Engine(MemorySegment handle, List<Object> adoptedCallbacks, Arena callbackArena) {
        if (handle == null || handle.address() == 0) {
            throw new DatalogicException("datalogic_engine_new returned null", null, null, null);
        }
        this.handle = new NativeHandle(this, handle, DatalogicNative.ENGINE_FREE, "Engine");
        this.callbackArena = callbackArena;
        if (adoptedCallbacks != null) {
            retainedCallbacks.addAll(adoptedCallbacks);
        }
    }

    private static MemorySegment newEngine(boolean templating) {
        try {
            return (MemorySegment) DatalogicNative.ENGINE_NEW.invokeExact(templating ? 1 : 0);
        } catch (Throwable t) {
            throw DatalogicException.propagate(t);
        }
    }

    /** The binding's version string (sourced from the underlying C ABI). */
    public static String version() {
        try {
            MemorySegment p = (MemorySegment) DatalogicNative.VERSION.invokeExact();
            // The one NUL-terminated string in the v2 ABI: a static
            // literal valid for the program's lifetime.
            return p.address() == 0 ? "" : p.reinterpret(Long.MAX_VALUE).getString(0);
        } catch (Throwable t) {
            throw DatalogicException.propagate(t);
        }
    }

    MemorySegment handle() {
        return handle.get();
    }

    /**
     * Compile a JSONLogic rule (as a JSON string) into a reusable
     * {@link Rule}.
     */
    public Rule compile(String ruleJson) {
        if (ruleJson == null) throw new NullPointerException("ruleJson");
        return compileWith(ruleJson, (rule, out, errSlot) ->
                (int) DatalogicNative.ENGINE_COMPILE.invokeExact(
                        handle(), rule, rule.byteSize(), out, errSlot));
    }

    /** One native compile entry point, called with the rule text and the out slots. */
    @FunctionalInterface
    private interface NativeCompile {
        int call(MemorySegment rule, MemorySegment out, MemorySegment errSlot) throws Throwable;
    }

    /** What every compile method shares: marshal the rule, call, wrap the handle. */
    private Rule compileWith(String ruleJson, NativeCompile compile) {
        try (Arena arena = Arena.ofConfined()) {
            MemorySegment rule = DatalogicNative.utf8(arena, ruleJson);
            MemorySegment out = arena.allocate(ValueLayout.ADDRESS);
            MemorySegment errSlot = arena.allocate(ValueLayout.ADDRESS);
            int status;
            try {
                status = compile.call(rule, out, errSlot);
            } catch (Throwable t) {
                throw DatalogicException.propagate(t);
            }
            if (status != DatalogicNative.STATUS_OK) {
                throw DatalogicException.fromNative(status, errSlot, "compile failed");
            }
            return new Rule(out.get(ValueLayout.ADDRESS, 0), this);
        } finally {
            Reference.reachabilityFence(this);
        }
    }

    /** Compile {@code ruleJson} in templating mode, whatever this engine's mode. */
    public Rule compileTemplate(String ruleJson) {
        return compileMode(ruleJson, CompileMode.TEMPLATE);
    }

    /** Compile {@code ruleJson} outside templating mode, whatever this engine's mode. */
    public Rule compileStrict(String ruleJson) {
        return compileMode(ruleJson, CompileMode.STRICT);
    }

    /** Compile {@code ruleJson} in {@code mode}, whatever mode this engine was built with. */
    public Rule compileMode(String ruleJson, CompileMode mode) {
        if (ruleJson == null) throw new NullPointerException("ruleJson");
        if (mode == null) throw new NullPointerException("mode");
        return compileWith(ruleJson, (rule, out, errSlot) ->
                (int) DatalogicNative.ENGINE_COMPILE_MODE.invokeExact(
                        handle(), rule, rule.byteSize(), mode.code(), out, errSlot));
    }

    /**
     * Compile {@code ruleJson}, refusing it if {@link #check} finds any
     * error: throws a {@link ParseException} with error type
     * {@code "CompileError"} whose {@link DatalogicException#diagnosticsJson()}
     * lists every problem.
     */
    public Rule compileChecked(String ruleJson) {
        if (ruleJson == null) throw new NullPointerException("ruleJson");
        return compileWith(ruleJson, (rule, out, errSlot) ->
                (int) DatalogicNative.ENGINE_COMPILE_CHECKED.invokeExact(
                        handle(), rule, rule.byteSize(), out, errSlot));
    }

    /**
     * Every problem this engine can see in {@code ruleJson} before it runs,
     * as a JSON array of {@code {code, severity, message, pointer, operator}}.
     * Finding problems does not throw: they are in the array.
     */
    public String check(String ruleJson, CompileMode mode) {
        if (ruleJson == null) throw new NullPointerException("ruleJson");
        if (mode == null) throw new NullPointerException("mode");
        try (Arena arena = Arena.ofConfined()) {
            MemorySegment rule = DatalogicNative.utf8(arena, ruleJson);
            MemorySegment buf = arena.allocate(DatalogicNative.BUF_LAYOUT);
            MemorySegment errSlot = arena.allocate(ValueLayout.ADDRESS);
            int status;
            try {
                status = (int) DatalogicNative.ENGINE_CHECK.invokeExact(
                        handle(), rule, rule.byteSize(), mode.code(), buf, errSlot);
            } catch (Throwable t) {
                throw DatalogicException.propagate(t);
            }
            if (status != DatalogicNative.STATUS_OK) {
                throw DatalogicException.fromNative(status, errSlot, "check failed");
            }
            return DatalogicNative.takeOwnedBuf(buf);
        } finally {
            Reference.reachabilityFence(this);
        }
    }

    /**
     * Every built-in operator this engine evaluates, as a JSON array in the
     * schema of the docs' {@code operators.json}.
     */
    public String operators() {
        try (Arena arena = Arena.ofConfined()) {
            MemorySegment buf = arena.allocate(DatalogicNative.BUF_LAYOUT);
            MemorySegment errSlot = arena.allocate(ValueLayout.ADDRESS);
            int status;
            try {
                status = (int) DatalogicNative.ENGINE_OPERATORS.invokeExact(handle(), buf, errSlot);
            } catch (Throwable t) {
                throw DatalogicException.propagate(t);
            }
            if (status != DatalogicNative.STATUS_OK) {
                throw DatalogicException.fromNative(status, errSlot, "operators failed");
            }
            return DatalogicNative.takeOwnedBuf(buf);
        } finally {
            Reference.reachabilityFence(this);
        }
    }

    /**
     * Whether the JSON {@code valueJson} is truthy under this engine's
     * configured truthiness. Under the default rules an empty object is
     * falsy, like an empty array.
     */
    public boolean truthy(String valueJson) {
        if (valueJson == null) throw new NullPointerException("valueJson");
        try (Arena arena = Arena.ofConfined()) {
            MemorySegment value = DatalogicNative.utf8(arena, valueJson);
            MemorySegment out = arena.allocate(ValueLayout.JAVA_INT);
            MemorySegment errSlot = arena.allocate(ValueLayout.ADDRESS);
            int status;
            try {
                status = (int) DatalogicNative.ENGINE_TRUTHY.invokeExact(
                        handle(), value, value.byteSize(), out, errSlot);
            } catch (Throwable t) {
                throw DatalogicException.propagate(t);
            }
            if (status != DatalogicNative.STATUS_OK) {
                throw DatalogicException.fromNative(status, errSlot, "truthy failed");
            }
            return out.get(ValueLayout.JAVA_INT, 0) != 0;
        } finally {
            Reference.reachabilityFence(this);
        }
    }

    /**
     * One-shot: compile and evaluate in a single call, returning the
     * result as a JSON-string. For repeated evaluations of the same rule,
     * prefer {@link #compile(String)} + {@link Rule#evaluate(String)}.
     */
    public String apply(String ruleJson, String dataJson) {
        if (ruleJson == null) throw new NullPointerException("ruleJson");
        if (dataJson == null) throw new NullPointerException("dataJson");
        try (Arena arena = Arena.ofConfined()) {
            MemorySegment rule = DatalogicNative.utf8(arena, ruleJson);
            MemorySegment data = DatalogicNative.utf8(arena, dataJson);
            MemorySegment buf = arena.allocate(DatalogicNative.BUF_LAYOUT);
            MemorySegment errSlot = arena.allocate(ValueLayout.ADDRESS);
            int status;
            try {
                status = (int) DatalogicNative.ENGINE_APPLY.invokeExact(
                        handle(), rule, rule.byteSize(), data, data.byteSize(), buf, errSlot);
            } catch (Throwable t) {
                throw DatalogicException.propagate(t);
            }
            if (status != DatalogicNative.STATUS_OK) {
                throw DatalogicException.fromNative(status, errSlot, "apply failed");
            }
            return DatalogicNative.takeOwnedBuf(buf);
        } finally {
            // apply() may dispatch custom operators: keep the upcall
            // stubs (reachable through this engine) alive across the call.
            Reference.reachabilityFence(this);
        }
    }

    /**
     * Open a hot-loop {@link Session} bound to this engine. Sessions are
     * NOT thread-safe — open one per thread.
     */
    public Session openSession() {
        MemorySegment s;
        try {
            s = (MemorySegment) DatalogicNative.ENGINE_SESSION.invokeExact(handle());
        } catch (Throwable t) {
            throw DatalogicException.propagate(t);
        } finally {
            Reference.reachabilityFence(this);
        }
        if (s.address() == 0) {
            throw new DatalogicException("datalogic_engine_session returned null", null, null, null);
        }
        return new Session(s, this);
    }

    /**
     * Open a {@link TracedSession} bound to this engine. Every
     * {@link TracedSession#evaluate(String, String)} returns a
     * {@link TracedRun} carrying the result alongside execution-step and
     * expression-tree metadata.
     */
    public TracedSession openTracedSession() {
        MemorySegment s;
        try {
            s = (MemorySegment) DatalogicNative.ENGINE_TRACED_SESSION.invokeExact(handle());
        } catch (Throwable t) {
            throw DatalogicException.propagate(t);
        } finally {
            Reference.reachabilityFence(this);
        }
        if (s.address() == 0) {
            throw new DatalogicException("datalogic_engine_traced_session returned null", null, null, null);
        }
        return new TracedSession(s, this);
    }

    /** Builder for engines with custom operators. */
    public static EngineBuilder builder() { return new EngineBuilder(); }

    @Override
    public void close() {
        handle.close();
        // retainedCallbacks / callbackArena stay referenced by this
        // object (and by rules/sessions created from it) — rules hold an
        // Arc on the Rust engine and may still dispatch custom
        // operators after the engine handle itself is freed.
    }
}
