// SPDX-License-Identifier: Apache-2.0

using System.Text.Json.Nodes;

using Goplasmatic.Datalogic.Native;

namespace Goplasmatic.Datalogic;

/// <summary>
/// A compiled JSONLogic rule, ready to evaluate against data. Safe to
/// share across threads — each <see cref="Evaluate"/> uses its own
/// short-lived arena. For tight loops, open a <see cref="Session"/> per
/// thread instead.
/// </summary>
public sealed class Rule : IDisposable
{
    private IntPtr _handle;

    internal Rule(IntPtr handle) { _handle = handle; }

    internal IntPtr Handle
    {
        get
        {
            if (_handle == IntPtr.Zero) throw new ObjectDisposedException(nameof(Rule));
            return _handle;
        }
    }

    /// <summary>
    /// Evaluate against <paramref name="dataJson"/> and return the result
    /// as a JSON-string.
    /// </summary>
    public string Evaluate(string dataJson)
    {
        ArgumentNullException.ThrowIfNull(dataJson);
        var ptr = NativeMethods.datalogic_rule_evaluate(Handle, dataJson);
        if (ptr == IntPtr.Zero)
        {
            throw DatalogicException.FromLastError("rule evaluate failed");
        }
        return NativeMethods.TakeUtf8String(ptr)!;
    }

    /// <summary>
    /// Variant of <see cref="Evaluate"/> that returns the parsed
    /// <see cref="JsonNode"/>.
    /// </summary>
    public JsonNode? EvaluateJson(string dataJson) => JsonNode.Parse(Evaluate(dataJson));

    /// <inheritdoc />
    public void Dispose()
    {
        if (_handle != IntPtr.Zero)
        {
            NativeMethods.datalogic_rule_free(_handle);
            _handle = IntPtr.Zero;
        }
        GC.SuppressFinalize(this);
    }

    /// <summary>Finaliser falls back to <see cref="Dispose"/>.</summary>
    ~Rule() { Dispose(); }
}
