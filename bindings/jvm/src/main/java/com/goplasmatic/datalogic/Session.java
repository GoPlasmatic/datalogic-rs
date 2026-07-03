/* SPDX-License-Identifier: Apache-2.0 */
package com.goplasmatic.datalogic;

import com.goplasmatic.datalogic.internal.DatalogicNative;

import java.lang.foreign.Arena;
import java.lang.foreign.MemorySegment;
import java.lang.foreign.ValueLayout;
import java.lang.invoke.MethodHandle;
import java.lang.ref.Reference;
import java.util.ArrayList;
import java.util.List;

/**
 * Hot-loop session bound to a single {@link Engine}. Reuses one
 * {@code bumpalo::Bump} across evaluations and resets it at the start of
 * every call so peak memory stays bounded. NOT thread-safe — open one
 * per thread.
 *
 * <p>Native session results are borrowed from a session-owned buffer;
 * every method here copies them into Java strings before returning, so
 * callers never observe the borrow.
 */
public final class Session implements AutoCloseable {
    private MemorySegment handle;
    // Keeps the owning engine's custom-operator stubs reachable while
    // evaluations (which may dispatch into Java) are in flight.
    private final Engine owner;

    Session(MemorySegment handle, Engine owner) {
        this.handle = handle;
        this.owner = owner;
    }

    private MemorySegment handle() {
        MemorySegment h = handle;
        if (h == null) throw new IllegalStateException("Session is closed");
        return h;
    }

    /**
     * Evaluate {@code rule} against {@code dataJson} using this session's
     * reusable arena.
     */
    public String evaluate(Rule rule, String dataJson) {
        if (rule == null) throw new NullPointerException("rule");
        if (dataJson == null) throw new NullPointerException("dataJson");
        try (Arena arena = Arena.ofConfined()) {
            MemorySegment data = DatalogicNative.utf8(arena, dataJson);
            MemorySegment outPtr = arena.allocate(ValueLayout.ADDRESS);
            MemorySegment outLen = arena.allocate(ValueLayout.JAVA_LONG);
            MemorySegment errSlot = arena.allocate(ValueLayout.ADDRESS);
            int status;
            try {
                status = (int) DatalogicNative.SESSION_EVALUATE.invokeExact(
                        handle(), rule.handle(), data, data.byteSize(), outPtr, outLen, errSlot);
            } catch (Throwable t) {
                throw DatalogicException.propagate(t);
            }
            if (status != DatalogicNative.STATUS_OK) {
                throw DatalogicException.fromNative(status, errSlot, "session evaluate failed");
            }
            // Borrowed result — copy before the next call touches this session.
            return DatalogicNative.readUtf8(
                    outPtr.get(ValueLayout.ADDRESS, 0), outLen.get(ValueLayout.JAVA_LONG, 0));
        } finally {
            Reference.reachabilityFence(rule);
            Reference.reachabilityFence(this);
        }
    }

    /**
     * Evaluate {@code rule} against a pre-parsed {@link DataHandle} —
     * the hot path: zero parse work per call.
     */
    public String evaluate(Rule rule, DataHandle data) {
        if (rule == null) throw new NullPointerException("rule");
        if (data == null) throw new NullPointerException("data");
        try (Arena arena = Arena.ofConfined()) {
            MemorySegment outPtr = arena.allocate(ValueLayout.ADDRESS);
            MemorySegment outLen = arena.allocate(ValueLayout.JAVA_LONG);
            MemorySegment errSlot = arena.allocate(ValueLayout.ADDRESS);
            int status;
            try {
                status = (int) DatalogicNative.SESSION_EVALUATE_DATA.invokeExact(
                        handle(), rule.handle(), data.handle(), outPtr, outLen, errSlot);
            } catch (Throwable t) {
                throw DatalogicException.propagate(t);
            }
            if (status != DatalogicNative.STATUS_OK) {
                throw DatalogicException.fromNative(status, errSlot, "session evaluate failed");
            }
            return DatalogicNative.readUtf8(
                    outPtr.get(ValueLayout.ADDRESS, 0), outLen.get(ValueLayout.JAVA_LONG, 0));
        } finally {
            Reference.reachabilityFence(rule);
            Reference.reachabilityFence(data);
            Reference.reachabilityFence(this);
        }
    }

    // =============== typed scalar results ===============

    /**
     * Evaluate and read the result as a strict JSON boolean. Throws
     * {@link EvaluateException} with error type {@code "TypeMismatch"}
     * if the result is any other JSON type; for JSONLogic truthiness
     * coercion use {@link #evaluateTruthy(Rule, DataHandle)}.
     */
    public boolean evaluateBool(Rule rule, DataHandle data) {
        return typedEval(DatalogicNative.SESSION_EVALUATE_BOOL, rule, data,
                ValueLayout.JAVA_INT, "evaluate_bool failed") != 0;
    }

    /**
     * Evaluate and read the result as an exact integer number. Throws
     * {@link EvaluateException} with error type {@code "TypeMismatch"}
     * when the result is not an exact integer (e.g. {@code 1.5} or a
     * string).
     */
    public long evaluateLong(Rule rule, DataHandle data) {
        return typedEval(DatalogicNative.SESSION_EVALUATE_I64, rule, data,
                ValueLayout.JAVA_LONG, "evaluate_i64 failed");
    }

    /**
     * Evaluate and read the result as a double. Accepts any JSON number;
     * throws {@link EvaluateException} with error type
     * {@code "TypeMismatch"} otherwise.
     */
    public double evaluateDouble(Rule rule, DataHandle data) {
        return Double.longBitsToDouble(typedEval(DatalogicNative.SESSION_EVALUATE_F64, rule, data,
                ValueLayout.JAVA_DOUBLE, "evaluate_f64 failed"));
    }

    /**
     * Evaluate and collapse the result to a boolean via the engine's
     * configured truthiness rules (the same coercion {@code if} /
     * {@code and} / {@code or} apply). Never type-mismatches — any
     * result truthy-converts.
     */
    public boolean evaluateTruthy(Rule rule, DataHandle data) {
        return typedEval(DatalogicNative.SESSION_EVALUATE_TRUTHY, rule, data,
                ValueLayout.JAVA_INT, "evaluate_truthy failed") != 0;
    }

    /**
     * Shared body of the four typed entry points. The scalar out-slot is
     * read before the call arena closes and returned as raw bits:
     * int32 zero-extended, int64 as-is, double via
     * {@link Double#doubleToRawLongBits(double)}.
     */
    private long typedEval(MethodHandle target, Rule rule, DataHandle data,
                           ValueLayout outLayout, String fallback) {
        if (rule == null) throw new NullPointerException("rule");
        if (data == null) throw new NullPointerException("data");
        try (Arena arena = Arena.ofConfined()) {
            // 8-byte slot covers int32_t, int64_t, and double out-params.
            MemorySegment out = arena.allocate(ValueLayout.JAVA_LONG);
            MemorySegment errSlot = arena.allocate(ValueLayout.ADDRESS);
            int status;
            try {
                status = (int) target.invokeExact(
                        handle(), rule.handle(), data.handle(), out, errSlot);
            } catch (Throwable t) {
                throw DatalogicException.propagate(t);
            }
            if (status != DatalogicNative.STATUS_OK) {
                throw DatalogicException.fromNative(status, errSlot, fallback);
            }
            if (outLayout == ValueLayout.JAVA_INT) {
                return Integer.toUnsignedLong(out.get(ValueLayout.JAVA_INT, 0));
            }
            if (outLayout == ValueLayout.JAVA_DOUBLE) {
                return Double.doubleToRawLongBits(out.get(ValueLayout.JAVA_DOUBLE, 0));
            }
            return out.get(ValueLayout.JAVA_LONG, 0);
        } finally {
            Reference.reachabilityFence(rule);
            Reference.reachabilityFence(data);
            Reference.reachabilityFence(this);
        }
    }

    // =============== batch ===============

    /**
     * Evaluate one rule against many pre-parsed payloads in a single
     * native call. Item failures never throw: each {@link EvalResult}
     * carries either the result JSON or that item's error info. Throws
     * only for call-level problems (closed handles, a rule from a
     * different engine, …).
     */
    public List<EvalResult> evaluateBatch(Rule rule, List<DataHandle> datas) {
        if (rule == null) throw new NullPointerException("rule");
        if (datas == null) throw new NullPointerException("datas");
        int n = datas.size();
        if (n == 0) return List.of();
        try (Arena arena = Arena.ofConfined()) {
            MemorySegment handles = arena.allocate(ValueLayout.ADDRESS, n);
            for (int i = 0; i < n; i++) {
                DataHandle d = datas.get(i);
                if (d == null) throw new NullPointerException("datas[" + i + "]");
                handles.setAtIndex(ValueLayout.ADDRESS, i, d.handle());
            }
            MemorySegment results = arena.allocate(DatalogicNative.SLICE_LAYOUT, n);
            MemorySegment statuses = arena.allocate(ValueLayout.JAVA_INT, n);
            MemorySegment errSlot = arena.allocate(ValueLayout.ADDRESS);
            int status;
            try {
                status = (int) DatalogicNative.SESSION_EVALUATE_BATCH.invokeExact(
                        handle(), rule.handle(), handles, (long) n, results, statuses, errSlot);
            } catch (Throwable t) {
                throw DatalogicException.propagate(t);
            }
            if (status != DatalogicNative.STATUS_OK) {
                throw DatalogicException.fromNative(status, errSlot, "evaluate_batch failed");
            }
            return collectItems(n, results, statuses);
        } finally {
            Reference.reachabilityFence(rule);
            Reference.reachabilityFence(datas);
            Reference.reachabilityFence(this);
        }
    }

    /**
     * Evaluate many rules against one pre-parsed payload in a single
     * native call — the rule-set / feature-flag shape. Same per-item
     * semantics as {@link #evaluateBatch(Rule, List)}.
     */
    public List<EvalResult> evaluateMany(List<Rule> rules, DataHandle data) {
        if (rules == null) throw new NullPointerException("rules");
        if (data == null) throw new NullPointerException("data");
        int n = rules.size();
        if (n == 0) return List.of();
        try (Arena arena = Arena.ofConfined()) {
            MemorySegment handles = arena.allocate(ValueLayout.ADDRESS, n);
            for (int i = 0; i < n; i++) {
                Rule r = rules.get(i);
                if (r == null) throw new NullPointerException("rules[" + i + "]");
                handles.setAtIndex(ValueLayout.ADDRESS, i, r.handle());
            }
            MemorySegment results = arena.allocate(DatalogicNative.SLICE_LAYOUT, n);
            MemorySegment statuses = arena.allocate(ValueLayout.JAVA_INT, n);
            MemorySegment errSlot = arena.allocate(ValueLayout.ADDRESS);
            int status;
            try {
                status = (int) DatalogicNative.SESSION_EVALUATE_MANY.invokeExact(
                        handle(), handles, (long) n, data.handle(), results, statuses, errSlot);
            } catch (Throwable t) {
                throw DatalogicException.propagate(t);
            }
            if (status != DatalogicNative.STATUS_OK) {
                throw DatalogicException.fromNative(status, errSlot, "evaluate_many failed");
            }
            return collectItems(n, results, statuses);
        } finally {
            Reference.reachabilityFence(rules);
            Reference.reachabilityFence(data);
            Reference.reachabilityFence(this);
        }
    }

    /** Copy the borrowed per-item slices into Java-side results. */
    private static List<EvalResult> collectItems(int n, MemorySegment results, MemorySegment statuses) {
        long sliceSize = DatalogicNative.SLICE_LAYOUT.byteSize();
        long ptrOffset = 0;
        long lenOffset = ValueLayout.ADDRESS.byteSize();
        List<EvalResult> items = new ArrayList<>(n);
        for (int i = 0; i < n; i++) {
            MemorySegment ptr = results.get(ValueLayout.ADDRESS, i * sliceSize + ptrOffset);
            long len = results.get(ValueLayout.JAVA_LONG, i * sliceSize + lenOffset);
            String payload = DatalogicNative.readUtf8(ptr, len);
            int itemStatus = statuses.getAtIndex(ValueLayout.JAVA_INT, i);
            items.add(itemStatus == DatalogicNative.STATUS_OK
                    ? EvalResult.success(payload)
                    : EvalResult.failure(payload));
        }
        return items;
    }

    /**
     * Manually reset the session's arena. Optional — every
     * {@link #evaluate(Rule, String)} already resets at the start of the
     * call.
     */
    public void reset() {
        try {
            DatalogicNative.SESSION_RESET.invokeExact(handle());
        } catch (Throwable t) {
            throw DatalogicException.propagate(t);
        }
    }

    /**
     * Bytes currently held by the session's arena (sum across all
     * chunks).
     */
    public long allocatedBytes() {
        try {
            return (long) DatalogicNative.SESSION_ALLOCATED_BYTES.invokeExact(handle());
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
                DatalogicNative.SESSION_FREE.invokeExact(h);
            } catch (Throwable t) {
                throw DatalogicException.propagate(t);
            }
        }
        Reference.reachabilityFence(owner);
    }
}
