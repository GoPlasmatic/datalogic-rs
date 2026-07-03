// SPDX-License-Identifier: Apache-2.0

using System.Text.Json;
using System.Text.Json.Nodes;

using Goplasmatic.Datalogic.Native;

namespace Goplasmatic.Datalogic;

/// <summary>
/// Hot-loop session bound to a single <see cref="Engine"/>. Reuses one
/// <c>bumpalo::Bump</c> arena and one result buffer across evaluations,
/// resetting them at the start of every call so peak memory stays
/// bounded. NOT thread-safe — open one per thread. Rules passed to a
/// session must come from the same engine that opened it.
/// </summary>
/// <remarks>
/// Native results are borrowed from the session's buffer and only valid
/// until the next call touching the session; every method here copies
/// them into managed strings before returning, so callers never see the
/// borrow.
/// </remarks>
public sealed class Session : IDisposable
{
    private IntPtr _handle;

    internal Session(IntPtr handle) { _handle = handle; }

    private IntPtr Handle
    {
        get
        {
            if (_handle == IntPtr.Zero) throw new ObjectDisposedException(nameof(Session));
            return _handle;
        }
    }

    /// <summary>
    /// Evaluate <paramref name="rule"/> against <paramref name="dataJson"/>
    /// using this session's reusable arena.
    /// </summary>
    public string Evaluate(Rule rule, string dataJson)
    {
        ArgumentNullException.ThrowIfNull(rule);
        ArgumentNullException.ThrowIfNull(dataJson);
        unsafe
        {
            using var dataU8 = Utf8Input.From(dataJson, stackalloc byte[Utf8Input.StackBufferSize]);
            var err = IntPtr.Zero;
            DatalogicStatus status;
            byte* outPtr;
            nuint outLen;
            fixed (byte* dp = dataU8.Span)
            {
                status = NativeMethods.datalogic_session_evaluate(
                    Handle, rule.Handle, dp, (nuint)dataU8.Span.Length, out outPtr, out outLen, ref err);
            }
            if (status != DatalogicStatus.Ok)
            {
                throw DatalogicException.FromNative(status, err, "session evaluate failed");
            }
            // Borrowed bytes — copy before anything else touches the session.
            var result = NativeMethods.BorrowedUtf8(outPtr, outLen);
            GC.KeepAlive(this);
            GC.KeepAlive(rule);
            return result;
        }
    }

    /// <summary>
    /// Evaluate <paramref name="rule"/> against a pre-parsed
    /// <see cref="DataHandle"/> — the hot path: zero data-parse work per
    /// call.
    /// </summary>
    public string Evaluate(Rule rule, DataHandle data)
    {
        ArgumentNullException.ThrowIfNull(rule);
        ArgumentNullException.ThrowIfNull(data);
        unsafe
        {
            var err = IntPtr.Zero;
            var status = NativeMethods.datalogic_session_evaluate_data(
                Handle, rule.Handle, data.Handle, out var outPtr, out var outLen, ref err);
            if (status != DatalogicStatus.Ok)
            {
                throw DatalogicException.FromNative(status, err, "session evaluate failed");
            }
            var result = NativeMethods.BorrowedUtf8(outPtr, outLen);
            GC.KeepAlive(this);
            GC.KeepAlive(rule);
            GC.KeepAlive(data);
            return result;
        }
    }

    /// <summary>
    /// Variant of <see cref="Evaluate(Rule,string)"/> returning a parsed
    /// <see cref="JsonNode"/>.
    /// </summary>
    public JsonNode? EvaluateJson(Rule rule, string dataJson) => JsonNode.Parse(Evaluate(rule, dataJson));

    /// <summary>
    /// Variant of <see cref="Evaluate(Rule,DataHandle)"/> returning a
    /// parsed <see cref="JsonNode"/>.
    /// </summary>
    public JsonNode? EvaluateJson(Rule rule, DataHandle data) => JsonNode.Parse(Evaluate(rule, data));

    // =============== typed scalar results ===============

    /// <summary>
    /// Evaluate and read the result as a strict JSON boolean. Throws
    /// <see cref="EvaluateException"/> with
    /// <see cref="EvaluationStatus.TypeMismatch"/> if the result is any
    /// other type; for JSONLogic truthiness coercion use
    /// <see cref="EvaluateTruthy"/>.
    /// </summary>
    public bool EvaluateBool(Rule rule, DataHandle data)
    {
        ArgumentNullException.ThrowIfNull(rule);
        ArgumentNullException.ThrowIfNull(data);
        var err = IntPtr.Zero;
        var status = NativeMethods.datalogic_session_evaluate_bool(
            Handle, rule.Handle, data.Handle, out var value, ref err);
        if (status != DatalogicStatus.Ok)
        {
            throw DatalogicException.FromNative(status, err, "session evaluate_bool failed");
        }
        KeepAlive(rule, data);
        return value != 0;
    }

    /// <summary>
    /// Evaluate and read the result as a 64-bit integer. Throws
    /// <see cref="EvaluateException"/> with
    /// <see cref="EvaluationStatus.TypeMismatch"/> when the result is
    /// not an exact integer number.
    /// </summary>
    public long EvaluateInt64(Rule rule, DataHandle data)
    {
        ArgumentNullException.ThrowIfNull(rule);
        ArgumentNullException.ThrowIfNull(data);
        var err = IntPtr.Zero;
        var status = NativeMethods.datalogic_session_evaluate_i64(
            Handle, rule.Handle, data.Handle, out var value, ref err);
        if (status != DatalogicStatus.Ok)
        {
            throw DatalogicException.FromNative(status, err, "session evaluate_i64 failed");
        }
        KeepAlive(rule, data);
        return value;
    }

    /// <summary>
    /// Evaluate and read the result as a double. Accepts any JSON
    /// number; throws <see cref="EvaluateException"/> with
    /// <see cref="EvaluationStatus.TypeMismatch"/> otherwise.
    /// </summary>
    public double EvaluateDouble(Rule rule, DataHandle data)
    {
        ArgumentNullException.ThrowIfNull(rule);
        ArgumentNullException.ThrowIfNull(data);
        var err = IntPtr.Zero;
        var status = NativeMethods.datalogic_session_evaluate_f64(
            Handle, rule.Handle, data.Handle, out var value, ref err);
        if (status != DatalogicStatus.Ok)
        {
            throw DatalogicException.FromNative(status, err, "session evaluate_f64 failed");
        }
        KeepAlive(rule, data);
        return value;
    }

    /// <summary>
    /// Evaluate and collapse the result to a boolean via the engine's
    /// configured truthiness rules (the same coercion <c>if</c> /
    /// <c>and</c> / <c>or</c> apply). Never type-mismatches — any result
    /// truthy-converts.
    /// </summary>
    public bool EvaluateTruthy(Rule rule, DataHandle data)
    {
        ArgumentNullException.ThrowIfNull(rule);
        ArgumentNullException.ThrowIfNull(data);
        var err = IntPtr.Zero;
        var status = NativeMethods.datalogic_session_evaluate_truthy(
            Handle, rule.Handle, data.Handle, out var value, ref err);
        if (status != DatalogicStatus.Ok)
        {
            throw DatalogicException.FromNative(status, err, "session evaluate_truthy failed");
        }
        KeepAlive(rule, data);
        return value != 0;
    }

    // =============== batch ===============

    /// <summary>
    /// Evaluate one rule against many pre-parsed payloads in a single
    /// native call. Per-item failures do not throw: each
    /// <see cref="EvaluationResult"/> carries either the result JSON or
    /// the item's error detail (tag, message, operator). The call itself
    /// only throws for argument-level problems (e.g. a rule from a
    /// different engine).
    /// </summary>
    public EvaluationResult[] EvaluateBatch(Rule rule, IReadOnlyList<DataHandle> datas)
    {
        ArgumentNullException.ThrowIfNull(rule);
        ArgumentNullException.ThrowIfNull(datas);
        var n = datas.Count;
        if (n == 0) return Array.Empty<EvaluationResult>();

        var dataPtrs = new IntPtr[n];
        for (var i = 0; i < n; i++)
        {
            var d = datas[i] ?? throw new ArgumentException($"datas[{i}] is null", nameof(datas));
            dataPtrs[i] = d.Handle;
        }

        var slices = new DatalogicSlice[n];
        var statuses = new DatalogicStatus[n];
        unsafe
        {
            var err = IntPtr.Zero;
            DatalogicStatus status;
            fixed (IntPtr* pd = dataPtrs)
            fixed (DatalogicSlice* pr = slices)
            fixed (DatalogicStatus* ps = statuses)
            {
                status = NativeMethods.datalogic_session_evaluate_batch(
                    Handle, rule.Handle, pd, (nuint)n, pr, ps, ref err);
            }
            if (status != DatalogicStatus.Ok)
            {
                throw DatalogicException.FromNative(status, err, "session batch evaluate failed");
            }

            // Every slice borrows from the session buffer — copy them
            // all out before anything else touches this session.
            var results = new EvaluationResult[n];
            for (var i = 0; i < n; i++)
            {
                results[i] = ItemResult(statuses[i], NativeMethods.BorrowedUtf8(slices[i].Ptr, slices[i].Len));
            }
            GC.KeepAlive(this);
            GC.KeepAlive(rule);
            GC.KeepAlive(datas);
            return results;
        }
    }

    /// <summary>
    /// Evaluate many rules against one pre-parsed payload in a single
    /// native call — the rule-set / feature-flag shape. Same per-item
    /// semantics as <see cref="EvaluateBatch"/>: item failures land in
    /// their <see cref="EvaluationResult"/> instead of throwing.
    /// </summary>
    public EvaluationResult[] EvaluateMany(IReadOnlyList<Rule> rules, DataHandle data)
    {
        ArgumentNullException.ThrowIfNull(rules);
        ArgumentNullException.ThrowIfNull(data);
        var n = rules.Count;
        if (n == 0) return Array.Empty<EvaluationResult>();

        var rulePtrs = new IntPtr[n];
        for (var i = 0; i < n; i++)
        {
            var r = rules[i] ?? throw new ArgumentException($"rules[{i}] is null", nameof(rules));
            rulePtrs[i] = r.Handle;
        }

        var slices = new DatalogicSlice[n];
        var statuses = new DatalogicStatus[n];
        unsafe
        {
            var err = IntPtr.Zero;
            DatalogicStatus status;
            fixed (IntPtr* pr = rulePtrs)
            fixed (DatalogicSlice* ps = slices)
            fixed (DatalogicStatus* pst = statuses)
            {
                status = NativeMethods.datalogic_session_evaluate_many(
                    Handle, pr, (nuint)n, data.Handle, ps, pst, ref err);
            }
            if (status != DatalogicStatus.Ok)
            {
                throw DatalogicException.FromNative(status, err, "session many evaluate failed");
            }

            var results = new EvaluationResult[n];
            for (var i = 0; i < n; i++)
            {
                results[i] = ItemResult(statuses[i], NativeMethods.BorrowedUtf8(slices[i].Ptr, slices[i].Len));
            }
            GC.KeepAlive(this);
            GC.KeepAlive(rules);
            GC.KeepAlive(data);
            return results;
        }
    }

    /// <summary>
    /// Convert one batch item (status + payload copied out of the
    /// session buffer) into an <see cref="EvaluationResult"/>. Failed
    /// items carry a small JSON object
    /// <c>{"tag": ..., "message": ..., "operator"?: ...}</c>.
    /// </summary>
    private static EvaluationResult ItemResult(DatalogicStatus status, string payload)
    {
        if (status == DatalogicStatus.Ok)
        {
            return EvaluationResult.Success(payload);
        }

        string? tag = null, message = null, op = null;
        try
        {
            using var doc = JsonDocument.Parse(payload);
            var root = doc.RootElement;
            if (root.ValueKind == JsonValueKind.Object)
            {
                if (root.TryGetProperty("tag", out var t)) tag = t.GetString();
                if (root.TryGetProperty("message", out var m)) message = m.GetString();
                if (root.TryGetProperty("operator", out var o)) op = o.GetString();
            }
        }
        catch (JsonException)
        {
            // Defensive: surface the raw payload if it isn't the
            // documented error object.
        }
        return EvaluationResult.Failure((EvaluationStatus)status, tag, message ?? payload, op);
    }

    private void KeepAlive(Rule rule, DataHandle data)
    {
        GC.KeepAlive(this);
        GC.KeepAlive(rule);
        GC.KeepAlive(data);
    }

    /// <summary>
    /// Manually reset the session's arena. Optional — every
    /// <see cref="Evaluate(Rule,string)"/> already resets at the start of
    /// the call.
    /// </summary>
    public void Reset() => NativeMethods.datalogic_session_reset(Handle);

    /// <summary>
    /// Bytes currently held by the session's arena (sum across all
    /// chunks). Useful for sizing or diagnostics.
    /// </summary>
    public nuint AllocatedBytes => NativeMethods.datalogic_session_allocated_bytes(Handle);

    /// <inheritdoc />
    public void Dispose()
    {
        if (_handle != IntPtr.Zero)
        {
            NativeMethods.datalogic_session_free(_handle);
            _handle = IntPtr.Zero;
        }
        GC.SuppressFinalize(this);
    }

    /// <summary>Finaliser falls back to <see cref="Dispose"/>.</summary>
    ~Session() { Dispose(); }
}
