// SPDX-License-Identifier: Apache-2.0

using System.Text.Json.Nodes;

using Goplasmatic.Datalogic.Native;

namespace Goplasmatic.Datalogic;

/// <summary>
/// A JSONLogic compile/evaluate engine. Wraps a shared
/// <c>Arc&lt;datalogic_rs::Engine&gt;</c> on the Rust side — safe to share
/// across threads. Dispose to release the native handle.
/// </summary>
/// <example>
/// <code>
/// using var engine = new Engine();
/// using var rule = engine.Compile("""{"var":"x"}""");
/// var result = rule.Evaluate("""{"x":42}""");  // "42"
/// </code>
/// </example>
public sealed class Engine : IDisposable
{
    private IntPtr _handle;
    // GCHandles for any custom-operator callbacks registered on this
    // engine. Released alongside the native handle on Dispose to keep
    // the delegates the native trampolines resolve alive until the
    // engine dies.
    private List<System.Runtime.InteropServices.GCHandle>? _pinnedCallbacks;

    static Engine() => NativeInit.EnsureLoaded();

    /// <summary>
    /// Construct an engine with default (non-templating) configuration.
    /// </summary>
    public Engine() : this(templating: false) { }

    /// <summary>
    /// Construct an engine, optionally enabling templating mode where
    /// multi-key objects in compiled rules become output-shaping
    /// templates instead of parse errors.
    /// </summary>
    public Engine(bool templating)
    {
        _handle = NativeMethods.datalogic_engine_new(templating ? 1 : 0);
        if (_handle == IntPtr.Zero)
        {
            throw new EvaluateException(
                "datalogic_engine_new returned NULL", null, null, null, EvaluationStatus.InternalError);
        }
    }

    internal Engine(IntPtr handle) { _handle = handle; }

    /// <summary>
    /// Adopt a set of GCHandles owned by a builder so they stay alive
    /// until this engine is disposed. Called once at construction time
    /// by <see cref="EngineBuilder.Build"/>.
    /// </summary>
    internal void AdoptPinnedCallbacks(List<System.Runtime.InteropServices.GCHandle> pinned)
    {
        _pinnedCallbacks = pinned;
    }

    internal IntPtr Handle
    {
        get
        {
            if (_handle == IntPtr.Zero) throw new ObjectDisposedException(nameof(Engine));
            return _handle;
        }
    }

    /// <summary>
    /// The binding's version string (sourced from the underlying C ABI,
    /// which tracks the datalogic-rs core exactly).
    /// </summary>
    public static string Version
    {
        get
        {
            NativeInit.EnsureLoaded();
            return NativeMethods.BorrowUtf8String(NativeMethods.datalogic_version()) ?? "";
        }
    }

    /// <summary>
    /// Compile a JSONLogic rule (as a JSON string) into a reusable
    /// <see cref="Rule"/> that can be evaluated against many inputs.
    /// </summary>
    /// <exception cref="ParseException">The rule JSON is malformed or uses an unknown operator.</exception>
    public Rule Compile(string ruleJson)
    {
        ArgumentNullException.ThrowIfNull(ruleJson);
        unsafe
        {
            using var ruleU8 = Utf8Input.From(ruleJson, stackalloc byte[Utf8Input.StackBufferSize]);
            var err = IntPtr.Zero;
            DatalogicStatus status;
            IntPtr rulePtr;
            fixed (byte* rp = ruleU8.Span)
            {
                status = NativeMethods.datalogic_engine_compile(
                    Handle, rp, (nuint)ruleU8.Span.Length, out rulePtr, ref err);
            }
            if (status != DatalogicStatus.Ok)
            {
                throw DatalogicException.FromNative(status, err, "compile failed");
            }
            GC.KeepAlive(this);
            return new Rule(rulePtr);
        }
    }

    /// <summary>Compile <paramref name="ruleJson"/> in templating mode, whatever this engine's mode.</summary>
    public Rule CompileTemplate(string ruleJson) => CompileMode(ruleJson, Datalogic.CompileMode.Template);

    /// <summary>Compile <paramref name="ruleJson"/> outside templating mode, whatever this engine's mode.</summary>
    public Rule CompileStrict(string ruleJson) => CompileMode(ruleJson, Datalogic.CompileMode.Strict);

    /// <summary>Compile <paramref name="ruleJson"/> in <paramref name="mode"/>, whatever this engine's mode.</summary>
    public Rule CompileMode(string ruleJson, CompileMode mode)
    {
        ArgumentNullException.ThrowIfNull(ruleJson);
        unsafe
        {
            using var ruleU8 = Utf8Input.From(ruleJson, stackalloc byte[Utf8Input.StackBufferSize]);
            var err = IntPtr.Zero;
            DatalogicStatus status;
            IntPtr rulePtr;
            fixed (byte* rp = ruleU8.Span)
            {
                status = NativeMethods.datalogic_engine_compile_mode(
                    Handle, rp, (nuint)ruleU8.Span.Length, (uint)mode, out rulePtr, ref err);
            }
            if (status != DatalogicStatus.Ok)
            {
                throw DatalogicException.FromNative(status, err, "compile failed");
            }
            GC.KeepAlive(this);
            return new Rule(rulePtr);
        }
    }

    /// <summary>
    /// Compile <paramref name="ruleJson"/>, refusing it if <see cref="Check"/> finds any
    /// error: throws a <see cref="ParseException"/> with <c>ErrorType</c>
    /// <c>"CompileError"</c> whose <see cref="DatalogicException.DiagnosticsJson"/> lists them.
    /// </summary>
    public Rule CompileChecked(string ruleJson)
    {
        ArgumentNullException.ThrowIfNull(ruleJson);
        unsafe
        {
            using var ruleU8 = Utf8Input.From(ruleJson, stackalloc byte[Utf8Input.StackBufferSize]);
            var err = IntPtr.Zero;
            DatalogicStatus status;
            IntPtr rulePtr;
            fixed (byte* rp = ruleU8.Span)
            {
                status = NativeMethods.datalogic_engine_compile_checked(
                    Handle, rp, (nuint)ruleU8.Span.Length, out rulePtr, ref err);
            }
            if (status != DatalogicStatus.Ok)
            {
                throw DatalogicException.FromNative(status, err, "compile failed");
            }
            GC.KeepAlive(this);
            return new Rule(rulePtr);
        }
    }

    /// <summary>
    /// Every problem this engine can see in <paramref name="ruleJson"/> before it runs,
    /// as a JSON array of <c>{code, severity, message, pointer, operator}</c>. Finding
    /// problems does not throw: they are in the array.
    /// </summary>
    public string Check(string ruleJson, CompileMode mode = Datalogic.CompileMode.Engine)
    {
        ArgumentNullException.ThrowIfNull(ruleJson);
        unsafe
        {
            using var ruleU8 = Utf8Input.From(ruleJson, stackalloc byte[Utf8Input.StackBufferSize]);
            var err = IntPtr.Zero;
            DatalogicStatus status;
            DatalogicBuf buf;
            fixed (byte* rp = ruleU8.Span)
            {
                status = NativeMethods.datalogic_engine_check(
                    Handle, rp, (nuint)ruleU8.Span.Length, (uint)mode, out buf, ref err);
            }
            if (status != DatalogicStatus.Ok)
            {
                throw DatalogicException.FromNative(status, err, "check failed");
            }
            GC.KeepAlive(this);
            return NativeMethods.TakeBufUtf8(buf);
        }
    }

    /// <summary>
    /// Every built-in operator this engine evaluates, as a JSON array in the schema of
    /// the docs' <c>operators.json</c>.
    /// </summary>
    public string Operators()
    {
        var err = IntPtr.Zero;
        var status = NativeMethods.datalogic_engine_operators(Handle, out var buf, ref err);
        if (status != DatalogicStatus.Ok)
        {
            throw DatalogicException.FromNative(status, err, "operators failed");
        }
        GC.KeepAlive(this);
        return NativeMethods.TakeBufUtf8(buf);
    }

    /// <summary>
    /// Whether the JSON <paramref name="valueJson"/> is truthy under this engine's
    /// configured truthiness. Under the default rules an empty object is falsy, like an
    /// empty array.
    /// </summary>
    public bool Truthy(string valueJson)
    {
        ArgumentNullException.ThrowIfNull(valueJson);
        unsafe
        {
            using var valueU8 = Utf8Input.From(valueJson, stackalloc byte[Utf8Input.StackBufferSize]);
            var err = IntPtr.Zero;
            DatalogicStatus status;
            int result;
            fixed (byte* vp = valueU8.Span)
            {
                status = NativeMethods.datalogic_engine_truthy(
                    Handle, vp, (nuint)valueU8.Span.Length, out result, ref err);
            }
            if (status != DatalogicStatus.Ok)
            {
                throw DatalogicException.FromNative(status, err, "truthy failed");
            }
            GC.KeepAlive(this);
            return result != 0;
        }
    }

    /// <summary>
    /// One-shot: compile and evaluate in a single call, returning the
    /// result as a JSON-string. Prefer <see cref="Compile"/> +
    /// <see cref="Rule.Evaluate(string)"/> for repeated evaluations of
    /// the same rule.
    /// </summary>
    public string Apply(string ruleJson, string dataJson)
    {
        ArgumentNullException.ThrowIfNull(ruleJson);
        ArgumentNullException.ThrowIfNull(dataJson);
        unsafe
        {
            using var ruleU8 = Utf8Input.From(ruleJson, stackalloc byte[Utf8Input.StackBufferSize]);
            using var dataU8 = Utf8Input.From(dataJson, stackalloc byte[Utf8Input.StackBufferSize]);
            var err = IntPtr.Zero;
            DatalogicStatus status;
            DatalogicBuf buf;
            fixed (byte* rp = ruleU8.Span)
            fixed (byte* dp = dataU8.Span)
            {
                status = NativeMethods.datalogic_engine_apply(
                    Handle,
                    rp, (nuint)ruleU8.Span.Length,
                    dp, (nuint)dataU8.Span.Length,
                    out buf, ref err);
            }
            if (status != DatalogicStatus.Ok)
            {
                throw DatalogicException.FromNative(status, err, "apply failed");
            }
            var result = NativeMethods.TakeBufUtf8(buf);
            GC.KeepAlive(this);
            return result;
        }
    }

    /// <summary>
    /// One-shot variant that returns the result as a parsed
    /// <see cref="JsonNode"/> rather than a JSON string.
    /// </summary>
    public JsonNode? ApplyJson(string ruleJson, string dataJson)
        => JsonNode.Parse(Apply(ruleJson, dataJson));

    /// <summary>
    /// Open a hot-loop <see cref="Session"/> bound to this engine. The
    /// session reuses one arena across evaluations and resets it at the
    /// start of every call to bound peak memory. Sessions are NOT
    /// thread-safe — open one per thread.
    /// </summary>
    public Session OpenSession()
    {
        var ptr = NativeMethods.datalogic_engine_session(Handle);
        if (ptr == IntPtr.Zero)
        {
            throw new EvaluateException(
                "datalogic_engine_session returned NULL", null, null, null, EvaluationStatus.InternalError);
        }
        GC.KeepAlive(this);
        return new Session(ptr);
    }

    /// <summary>
    /// Open a <see cref="TracedSession"/> bound to this engine. Every
    /// <c>Evaluate</c> call returns the result alongside the execution
    /// step log and expression tree (see <see cref="TracedRun"/>).
    /// </summary>
    public TracedSession OpenTracedSession()
    {
        var ptr = NativeMethods.datalogic_engine_traced_session(Handle);
        if (ptr == IntPtr.Zero)
        {
            throw new EvaluateException(
                "datalogic_engine_traced_session returned NULL", null, null, null, EvaluationStatus.InternalError);
        }
        GC.KeepAlive(this);
        return new TracedSession(ptr);
    }

    /// <summary>
    /// Construct a new <see cref="EngineBuilder"/> for registering custom
    /// JSONLogic operators implemented in C# before building the engine.
    /// </summary>
    public static EngineBuilder Builder() => new();

    /// <inheritdoc />
    public void Dispose()
    {
        if (_handle != IntPtr.Zero)
        {
            NativeMethods.datalogic_engine_free(_handle);
            _handle = IntPtr.Zero;
        }
        if (_pinnedCallbacks is not null)
        {
            foreach (var h in _pinnedCallbacks) h.Free();
            _pinnedCallbacks = null;
        }
        GC.SuppressFinalize(this);
    }

    /// <summary>Finaliser falls back to <see cref="Dispose"/>.</summary>
    ~Engine() { Dispose(); }
}
