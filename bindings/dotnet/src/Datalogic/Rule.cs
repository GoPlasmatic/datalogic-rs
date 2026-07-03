// SPDX-License-Identifier: Apache-2.0

using System.Text.Json.Nodes;

using Goplasmatic.Datalogic.Native;

namespace Goplasmatic.Datalogic;

/// <summary>
/// A compiled JSONLogic rule, ready to evaluate against data. Safe to
/// share across threads — each <see cref="Evaluate(string)"/> uses its
/// own pooled arena. For tight loops, open a <see cref="Session"/> per
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
        unsafe
        {
            using var dataU8 = Utf8Input.From(dataJson, stackalloc byte[Utf8Input.StackBufferSize]);
            var err = IntPtr.Zero;
            DatalogicStatus status;
            DatalogicBuf buf;
            fixed (byte* dp = dataU8.Span)
            {
                status = NativeMethods.datalogic_rule_evaluate(
                    Handle, dp, (nuint)dataU8.Span.Length, out buf, ref err);
            }
            if (status != DatalogicStatus.Ok)
            {
                throw DatalogicException.FromNative(status, err, "rule evaluate failed");
            }
            var result = NativeMethods.TakeBufUtf8(buf);
            GC.KeepAlive(this);
            return result;
        }
    }

    /// <summary>
    /// Evaluate against a pre-parsed <see cref="DataHandle"/> and return
    /// the result as a JSON-string — no data parse work per call. Both
    /// this rule and <paramref name="data"/> are thread-safe, so this
    /// call can run concurrently from many threads.
    /// </summary>
    public string Evaluate(DataHandle data)
    {
        ArgumentNullException.ThrowIfNull(data);
        var err = IntPtr.Zero;
        var status = NativeMethods.datalogic_rule_evaluate_data(Handle, data.Handle, out var buf, ref err);
        if (status != DatalogicStatus.Ok)
        {
            throw DatalogicException.FromNative(status, err, "rule evaluate failed");
        }
        var result = NativeMethods.TakeBufUtf8(buf);
        GC.KeepAlive(this);
        GC.KeepAlive(data);
        return result;
    }

    /// <summary>
    /// Variant of <see cref="Evaluate(string)"/> that returns the parsed
    /// <see cref="JsonNode"/>.
    /// </summary>
    public JsonNode? EvaluateJson(string dataJson) => JsonNode.Parse(Evaluate(dataJson));

    /// <summary>
    /// Variant of <see cref="Evaluate(DataHandle)"/> that returns the
    /// parsed <see cref="JsonNode"/>.
    /// </summary>
    public JsonNode? EvaluateJson(DataHandle data) => JsonNode.Parse(Evaluate(data));

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
