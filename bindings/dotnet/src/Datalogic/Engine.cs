// SPDX-License-Identifier: Apache-2.0

using System.Text.Json;
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
    // Pinned GCHandles for any custom-operator callbacks registered on
    // this engine. Released alongside the native handle on Dispose to
    // keep the C-side function pointers valid until the engine dies.
    private List<System.Runtime.InteropServices.GCHandle>? _pinnedCallbacks;

    static Engine() => NativeLibraryResolver.Install();

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
            throw DatalogicException.FromLastError("datalogic_engine_new returned NULL");
        }
    }

    internal Engine(IntPtr handle) { _handle = handle; }

    /// <summary>
    /// Adopt a set of pinned GCHandles owned by a builder so they stay
    /// alive until this engine is disposed. Called once at construction
    /// time by <see cref="EngineBuilder.Build"/>.
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
            NativeLibraryResolver.Install();
            return NativeMethods.BorrowUtf8String(NativeMethods.datalogic_version()) ?? "";
        }
    }

    /// <summary>
    /// Compile a JSONLogic rule (as a JSON string) into a reusable
    /// <see cref="Rule"/> that can be evaluated against many inputs.
    /// </summary>
    public Rule Compile(string ruleJson)
    {
        ArgumentNullException.ThrowIfNull(ruleJson);
        var rule = NativeMethods.datalogic_engine_compile(Handle, ruleJson);
        if (rule == IntPtr.Zero)
        {
            throw DatalogicException.FromLastError("compile failed");
        }
        return new Rule(rule);
    }

    /// <summary>
    /// One-shot: compile and evaluate in a single call, returning the
    /// result as a JSON-string. Prefer <see cref="Compile"/> +
    /// <see cref="Rule.Evaluate"/> for repeated evaluations of the same
    /// rule.
    /// </summary>
    public string Apply(string ruleJson, string dataJson)
    {
        ArgumentNullException.ThrowIfNull(ruleJson);
        ArgumentNullException.ThrowIfNull(dataJson);
        var ptr = NativeMethods.datalogic_engine_apply(Handle, ruleJson, dataJson);
        if (ptr == IntPtr.Zero)
        {
            throw DatalogicException.FromLastError("apply failed");
        }
        return NativeMethods.TakeUtf8String(ptr)!;
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
            throw DatalogicException.FromLastError("session failed");
        }
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
            throw DatalogicException.FromLastError("traced session failed");
        }
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
