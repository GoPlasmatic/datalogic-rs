/* SPDX-License-Identifier: Apache-2.0 */
package com.goplasmatic.datalogic;

import com.goplasmatic.datalogic.internal.DatalogicNative;
import com.sun.jna.Pointer;

import java.util.ArrayList;
import java.util.List;

/**
 * A JSONLogic compile/evaluate engine. Wraps a shared
 * {@code Arc<datalogic_rs::Engine>} on the Rust side and is safe to
 * share across threads. {@link #close()} releases the native handle.
 *
 * <pre>
 * try (Engine engine = new Engine()) {
 *     String result = engine.apply("{\"+\":[1,2]}", "{}");  // "3"
 * }
 * </pre>
 */
public class Engine implements AutoCloseable {
    private Pointer handle;
    // JNA holds Callback references weakly via the C side; hold them
    // strongly here so they live as long as the engine.
    final List<DatalogicNative.OperatorCallback> retainedCallbacks = new ArrayList<>();

    /** Construct an engine with default (non-templating) configuration. */
    public Engine() {
        this(false);
    }

    /**
     * Construct an engine, optionally enabling templating mode where
     * multi-key objects in compiled rules become output-shaping templates.
     */
    public Engine(boolean templating) {
        this(DatalogicNative.INSTANCE.datalogic_engine_new(templating ? 1 : 0), null);
    }

    Engine(Pointer handle, List<DatalogicNative.OperatorCallback> adoptedCallbacks) {
        this.handle = handle;
        if (handle == null || Pointer.nativeValue(handle) == 0) {
            throw DatalogicException.fromLastError("datalogic_engine_new returned null");
        }
        if (adoptedCallbacks != null) {
            retainedCallbacks.addAll(adoptedCallbacks);
        }
    }

    /** The binding's version string (sourced from the underlying C ABI). */
    public static String version() {
        Pointer p = DatalogicNative.INSTANCE.datalogic_version();
        return p == null ? "" : p.getString(0, "UTF-8");
    }

    Pointer handle() {
        if (handle == null) throw new IllegalStateException("Engine is closed");
        return handle;
    }

    /**
     * Compile a JSONLogic rule (as a JSON string) into a reusable
     * {@link Rule}.
     */
    public Rule compile(String ruleJson) {
        if (ruleJson == null) throw new NullPointerException("ruleJson");
        Pointer r = DatalogicNative.INSTANCE.datalogic_engine_compile(handle(), ruleJson);
        if (r == null) throw DatalogicException.fromLastError("compile failed");
        return new Rule(r);
    }

    /**
     * One-shot: compile and evaluate in a single call, returning the
     * result as a JSON-string. For repeated evaluations of the same rule,
     * prefer {@link #compile(String)} + {@link Rule#evaluate(String)}.
     */
    public String apply(String ruleJson, String dataJson) {
        if (ruleJson == null) throw new NullPointerException("ruleJson");
        if (dataJson == null) throw new NullPointerException("dataJson");
        Pointer ptr = DatalogicNative.INSTANCE.datalogic_engine_apply(handle(), ruleJson, dataJson);
        if (ptr == null) throw DatalogicException.fromLastError("apply failed");
        return takeOwnedString(ptr);
    }

    /**
     * Open a hot-loop {@link Session} bound to this engine. Sessions are
     * NOT thread-safe — open one per thread.
     */
    public Session openSession() {
        Pointer s = DatalogicNative.INSTANCE.datalogic_engine_session(handle());
        if (s == null) throw DatalogicException.fromLastError("session failed");
        return new Session(s);
    }

    /**
     * Open a {@link TracedSession} bound to this engine. Every
     * {@link TracedSession#evaluate(String, String)} returns a
     * {@link TracedRun} carrying the result alongside execution-step and
     * expression-tree metadata.
     */
    public TracedSession openTracedSession() {
        Pointer s = DatalogicNative.INSTANCE.datalogic_engine_traced_session(handle());
        if (s == null) throw DatalogicException.fromLastError("traced session failed");
        return new TracedSession(s);
    }

    /** Builder for engines with custom operators. */
    public static EngineBuilder builder() { return new EngineBuilder(); }

    @Override
    public void close() {
        if (handle != null) {
            DatalogicNative.INSTANCE.datalogic_engine_free(handle);
            handle = null;
        }
        retainedCallbacks.clear();
    }

    /**
     * Marshal a returned UTF-8 C string (callee-owned) into a Java
     * String and free the native allocation.
     */
    static String takeOwnedString(Pointer ptr) {
        String s = ptr.getString(0, "UTF-8");
        DatalogicNative.INSTANCE.datalogic_string_free(ptr);
        return s;
    }
}
