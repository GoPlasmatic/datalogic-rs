// SPDX-License-Identifier: Apache-2.0

using System.Runtime.InteropServices;
using System.Text;

using Goplasmatic.Datalogic.Native;

namespace Goplasmatic.Datalogic;

/// <summary>
/// Custom JSONLogic operator implemented in C#. The argument JSON is
/// the operator's pre-evaluated arguments as a JSON-array string
/// (e.g. <c>"[1, 2, \"x\"]"</c>); return the operator's result as a
/// JSON-value string (e.g. <c>"42"</c>, <c>"\"a\""</c>,
/// <c>"{\"k\":1}"</c>). Throw to signal an evaluation error — the
/// exception's message bubbles back to the caller.
/// </summary>
public delegate string CustomOperator(string argsJson);

/// <summary>
/// Builder for engines with custom operators. Mirrors the cross-binding
/// contract (registering a name that collides with a built-in like
/// <c>+</c> / <c>if</c> / <c>var</c> silently dispatches to the built-in
/// — built-ins always win).
/// </summary>
public sealed class EngineBuilder
{
    private IntPtr _handle;
    private bool _consumed;
    // Keep every registered callback delegate alive (GCHandle) until
    // the resulting Engine is disposed — the native side stores the
    // GCHandle address as its user_data.
    private readonly List<GCHandle> _pinned = new();

    static EngineBuilder() => NativeInit.EnsureLoaded();

    /// <summary>Construct a fresh, empty builder.</summary>
    public EngineBuilder()
    {
        _handle = NativeMethods.datalogic_engine_builder_new();
        if (_handle == IntPtr.Zero)
        {
            throw new EvaluateException(
                "datalogic_engine_builder_new returned NULL", null, null, null, EvaluationStatus.InternalError);
        }
    }

    /// <summary>
    /// Toggle templating mode on the resulting engine.
    /// </summary>
    public EngineBuilder WithTemplating(bool enabled)
    {
        EnsureFresh();
        NativeMethods.datalogic_engine_builder_set_templating(_handle, enabled ? 1 : 0);
        return this;
    }

    /// <summary>
    /// Set the engine's evaluation configuration from a JSON object
    /// string, parsed by the core crate's shared config parser (the same
    /// wire format every binding uses). All keys are optional; an
    /// optional <c>"preset"</c> (<c>"default"</c> |
    /// <c>"safe_arithmetic"</c> | <c>"strict"</c>) selects the starting
    /// point and the remaining keys (<c>arithmetic_nan_handling</c>,
    /// <c>division_by_zero</c>, <c>loose_equality_errors</c>,
    /// <c>truthy_evaluator</c>, <c>numeric_coercion</c> as an object of
    /// bools, <c>max_recursion_depth</c>) override individual fields on
    /// top of it. Unknown keys and values are rejected (error type
    /// <c>"ConfigurationError"</c>) so typos fail loudly instead of
    /// being silently ignored. Each call replaces the builder's entire
    /// evaluation config; templating and registered operators are
    /// unaffected.
    /// </summary>
    /// <exception cref="EvaluateException">
    /// The config JSON is malformed or contains unknown keys or values.
    /// </exception>
    public EngineBuilder SetConfigJson(string json)
    {
        ArgumentNullException.ThrowIfNull(json);
        EnsureFresh();
        unsafe
        {
            using var jsonU8 = Utf8Input.From(json, stackalloc byte[Utf8Input.StackBufferSize]);
            var err = IntPtr.Zero;
            DatalogicStatus status;
            fixed (byte* jp = jsonU8.Span)
            {
                status = NativeMethods.datalogic_engine_builder_set_config_json(
                    _handle, jp, (nuint)jsonU8.Span.Length, ref err);
            }
            if (status != DatalogicStatus.Ok)
            {
                throw DatalogicException.FromNative(status, err, "set_config_json failed");
            }
        }
        return this;
    }

    /// <summary>
    /// Register a custom JSONLogic operator under <paramref name="name"/>.
    /// The callback may be invoked from any thread that evaluates rules
    /// on the built engine, so it must be thread-safe.
    /// </summary>
    public EngineBuilder AddOperator(string name, CustomOperator op)
    {
        ArgumentException.ThrowIfNullOrEmpty(name);
        ArgumentNullException.ThrowIfNull(op);
        EnsureFresh();

        // A GCHandle keeps the delegate reachable while the engine is
        // alive; its IntPtr form rides across the boundary as the
        // callback's user_data. Released when the owning Engine is
        // disposed.
        var handle = GCHandle.Alloc(op);
        _pinned.Add(handle);

        unsafe
        {
            delegate* unmanaged[Cdecl]<byte*, nuint, void*, IntPtr, int> trampoline = &Trampoline;
            using var nameU8 = Utf8Input.From(name, stackalloc byte[Utf8Input.StackBufferSize]);
            var err = IntPtr.Zero;
            DatalogicStatus status;
            fixed (byte* np = nameU8.Span)
            {
                status = NativeMethods.datalogic_engine_builder_add_operator(
                    _handle,
                    np, (nuint)nameU8.Span.Length,
                    (IntPtr)trampoline,
                    GCHandle.ToIntPtr(handle),
                    ref err);
            }
            if (status != DatalogicStatus.Ok)
            {
                throw DatalogicException.FromNative(status, err, "add_operator failed");
            }
        }
        return this;
    }

    /// <summary>
    /// Finalise the builder into an <see cref="Engine"/>. The builder is
    /// consumed; subsequent calls throw.
    /// </summary>
    public Engine Build()
    {
        EnsureFresh();
        var enginePtr = NativeMethods.datalogic_engine_builder_build(_handle);
        // Per the C ABI contract, the builder handle still needs freeing
        // after build() drains it.
        NativeMethods.datalogic_engine_builder_free(_handle);
        _handle = IntPtr.Zero;
        _consumed = true;

        if (enginePtr == IntPtr.Zero)
        {
            ReleasePins();
            throw new EvaluateException(
                "builder build failed", null, null, null, EvaluationStatus.InternalError);
        }
        var engine = new Engine(enginePtr);
        // Ownership of pin list transfers to the engine; the builder
        // keeps its `_pinned` list as the same reference but treats it
        // as immutable from here (builder is consumed, can't add more).
        engine.AdoptPinnedCallbacks(_pinned);
        return engine;
    }

    private void EnsureFresh()
    {
        if (_consumed) throw new InvalidOperationException("EngineBuilder has already been built.");
        if (_handle == IntPtr.Zero) throw new ObjectDisposedException(nameof(EngineBuilder));
    }

    private void ReleasePins()
    {
        foreach (var h in _pinned) h.Free();
        _pinned.Clear();
    }

    /// <summary>
    /// `extern "C"` trampoline invoked by the Rust engine for every
    /// custom-operator call (v2 `datalogic_op_fn` contract). Resolves
    /// the <see cref="GCHandle"/> back to the C# delegate, forwards the
    /// call, and writes the outcome through
    /// `datalogic_op_result_set_json` / `_set_error` (both copy
    /// immediately — nothing allocated here crosses the boundary).
    /// Returns 0 on success, non-zero on failure.
    /// </summary>
    [UnmanagedCallersOnly(CallConvs = new[] { typeof(System.Runtime.CompilerServices.CallConvCdecl) })]
    private static unsafe int Trampoline(byte* argsJson, nuint argsLen, void* userData, IntPtr outResult)
    {
        try
        {
            var handle = GCHandle.FromIntPtr((IntPtr)userData);
            if (handle.Target is not CustomOperator op)
            {
                SetError(outResult, "internal: operator handle had wrong type");
                return 1;
            }
            var args = argsJson == null || argsLen == 0
                ? "[]"
                : Encoding.UTF8.GetString(argsJson, checked((int)argsLen));
            var result = op(args);
            if (result is null)
            {
                SetError(outResult, "custom operator returned null");
                return 1;
            }
            var bytes = Encoding.UTF8.GetBytes(result);
            fixed (byte* rp = bytes)
            {
                NativeMethods.datalogic_op_result_set_json(outResult, rp, (nuint)bytes.Length);
            }
            return 0;
        }
        catch (Exception ex)
        {
            SetError(outResult, ex.Message);
            return 1;
        }
    }

    private static unsafe void SetError(IntPtr outResult, string message)
    {
        try
        {
            var bytes = Encoding.UTF8.GetBytes(message);
            fixed (byte* mp = bytes)
            {
                NativeMethods.datalogic_op_result_set_error(outResult, mp, (nuint)bytes.Length);
            }
        }
        catch
        {
            // Best effort: a non-zero return with no message still
            // produces a generic engine error naming the operator.
        }
    }
}
