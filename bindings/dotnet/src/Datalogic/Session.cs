// SPDX-License-Identifier: Apache-2.0

using System.Text.Json.Nodes;

using Goplasmatic.Datalogic.Native;

namespace Goplasmatic.Datalogic;

/// <summary>
/// Hot-loop session bound to a single <see cref="Engine"/>. Reuses one
/// <c>bumpalo::Bump</c> across evaluations and resets it at the start of
/// every call so peak memory stays bounded. NOT thread-safe — open one
/// per thread.
/// </summary>
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
        var ptr = NativeMethods.datalogic_session_evaluate(Handle, rule.Handle, dataJson);
        if (ptr == IntPtr.Zero)
        {
            throw DatalogicException.FromLastError("session evaluate failed");
        }
        return NativeMethods.TakeUtf8String(ptr)!;
    }

    /// <summary>
    /// Variant of <see cref="Evaluate(Rule,string)"/> returning a parsed
    /// <see cref="JsonNode"/>.
    /// </summary>
    public JsonNode? EvaluateJson(Rule rule, string dataJson) => JsonNode.Parse(Evaluate(rule, dataJson));

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
