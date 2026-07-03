// SPDX-License-Identifier: Apache-2.0

using System.Text.Json;
using System.Text.Json.Nodes;

using Goplasmatic.Datalogic.Native;

namespace Goplasmatic.Datalogic;

/// <summary>
/// Result of a traced evaluation. Mirrors the cross-binding wire JSON
/// shape: <c>{result, expression_tree, steps, error?, structured_error?}</c>.
/// </summary>
public sealed class TracedRun
{
    /// <summary>Evaluation result, or <c>null</c> if the run errored.</summary>
    public JsonNode? Result { get; init; }
    /// <summary>Compile-time expression tree for flow-diagram rendering.</summary>
    public JsonNode? ExpressionTree { get; init; }
    /// <summary>Per-node execution log captured during the run.</summary>
    public JsonArray Steps { get; init; } = new();
    /// <summary>Engine error message, or <c>null</c> on success.</summary>
    public string? Error { get; init; }
    /// <summary>Structured error object, or <c>null</c> on success.</summary>
    public JsonNode? StructuredError { get; init; }

    /// <summary>Whether the run succeeded.</summary>
    public bool IsSuccess => Error is null;
}

/// <summary>
/// Trace-enabled handle over an <see cref="Engine"/>. Every
/// <see cref="Evaluate"/> call returns a <see cref="TracedRun"/>
/// carrying the result alongside execution-step and expression-tree
/// metadata. The handle is thread-safe — share freely.
/// </summary>
public sealed class TracedSession : IDisposable
{
    private IntPtr _handle;

    internal TracedSession(IntPtr handle) { _handle = handle; }

    private IntPtr Handle
    {
        get
        {
            if (_handle == IntPtr.Zero) throw new ObjectDisposedException(nameof(TracedSession));
            return _handle;
        }
    }

    /// <summary>
    /// One-shot traced evaluation: compile <paramref name="ruleJson"/>
    /// internally with the optimizer disabled, evaluate against
    /// <paramref name="dataJson"/>, and return the result + trace.
    /// </summary>
    /// <remarks>
    /// Engine errors surface inside the returned <see cref="TracedRun"/>
    /// (<see cref="TracedRun.Error"/>) rather than as a thrown exception
    /// — the trace data is always returned alongside, even on failure.
    /// Use <see cref="TracedRun.IsSuccess"/> to branch.
    /// </remarks>
    public TracedRun Evaluate(string ruleJson, string dataJson)
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
                status = NativeMethods.datalogic_traced_session_evaluate(
                    Handle,
                    rp, (nuint)ruleU8.Span.Length,
                    dp, (nuint)dataU8.Span.Length,
                    out buf, ref err);
            }
            // Engine errors surface inside the returned JSON payload with
            // an Ok status; a non-Ok status here means invalid arguments.
            if (status != DatalogicStatus.Ok)
            {
                throw DatalogicException.FromNative(status, err, "traced session evaluate failed");
            }
            var json = NativeMethods.TakeBufUtf8(buf);
            GC.KeepAlive(this);
            return Parse(json);
        }
    }

    private static TracedRun Parse(string json)
    {
        var doc = JsonNode.Parse(json) as JsonObject
                  ?? throw new EvaluateException("traced session returned non-object payload", null, null, null);

        return new TracedRun
        {
            Result = doc["result"]?.DeepClone(),
            ExpressionTree = doc["expression_tree"]?.DeepClone(),
            Steps = doc["steps"] is JsonArray arr ? (JsonArray)arr.DeepClone() : new JsonArray(),
            Error = doc["error"]?.GetValue<string>(),
            StructuredError = doc["structured_error"]?.DeepClone(),
        };
    }

    /// <inheritdoc />
    public void Dispose()
    {
        if (_handle != IntPtr.Zero)
        {
            NativeMethods.datalogic_traced_session_free(_handle);
            _handle = IntPtr.Zero;
        }
        GC.SuppressFinalize(this);
    }

    /// <summary>Finaliser falls back to <see cref="Dispose"/>.</summary>
    ~TracedSession() { Dispose(); }
}
