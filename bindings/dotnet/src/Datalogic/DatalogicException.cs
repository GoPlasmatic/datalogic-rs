// SPDX-License-Identifier: Apache-2.0

using System.Text;

using Goplasmatic.Datalogic.Native;

namespace Goplasmatic.Datalogic;

/// <summary>
/// Base class for every exception thrown by this binding. Carries the
/// structured error detail exposed by the C ABI's error handles
/// (`datalogic_error_*` accessors).
/// </summary>
public class DatalogicException : Exception
{
    /// <summary>Stable error tag from the engine (e.g. "ParseError", "Thrown", "NaN", "TypeMismatch").</summary>
    public string? ErrorType { get; }
    /// <summary>Outermost failing operator name (e.g. "+"), or null if not operator-scoped.</summary>
    public string? Operator { get; }
    /// <summary>Resolved root-to-leaf error path as a JSON array, or null if not available.</summary>
    public string? PathJson { get; }
    /// <summary>Coarse status the failing native call returned.</summary>
    public EvaluationStatus Status { get; }

    internal DatalogicException(
        string message,
        string? errorType,
        string? @operator,
        string? pathJson,
        EvaluationStatus status = EvaluationStatus.EvaluationError)
        : base(message)
    {
        ErrorType = errorType;
        Operator = @operator;
        PathJson = pathJson;
        Status = status;
    }

    /// <summary>
    /// Consume a `datalogic_error *` handle produced by a failing native
    /// call: read the accessors, ALWAYS free the handle, and construct
    /// the mapped exception subclass. <paramref name="err"/> may be
    /// <see cref="IntPtr.Zero"/> (no capture) — then the
    /// <paramref name="fallback"/> message and the raw status drive the
    /// mapping.
    /// </summary>
    internal static DatalogicException FromNative(DatalogicStatus status, IntPtr err, string fallback)
    {
        var message = fallback;
        string? tag = null, op = null, path = null;
        if (err != IntPtr.Zero)
        {
            try
            {
                // The handle's own status is authoritative (identical to
                // the returned one by construction).
                status = NativeMethods.datalogic_error_status(err);
                unsafe
                {
                    message = Read(NativeMethods.datalogic_error_message(err, out var len), len) ?? fallback;
                    tag = Read(NativeMethods.datalogic_error_tag(err, out len), len);
                    op = Read(NativeMethods.datalogic_error_operator(err, out len), len);
                    path = Read(NativeMethods.datalogic_error_path_json(err, out len), len);
                }
            }
            finally
            {
                NativeMethods.datalogic_error_free(err);
            }
        }
        return Create((EvaluationStatus)status, tag, message, op, path);
    }

    /// <summary>
    /// Map (status, tag) to the public exception subclass: parse
    /// failures become <see cref="ParseException"/>, everything else
    /// <see cref="EvaluateException"/> — the same split the v1 binding
    /// exposed.
    /// </summary>
    internal static DatalogicException Create(
        EvaluationStatus status,
        string? tag,
        string message,
        string? @operator,
        string? pathJson)
        => status == EvaluationStatus.ParseError || tag == "ParseError"
            ? new ParseException(message, tag, @operator, pathJson, status)
            : new EvaluateException(message, tag, @operator, pathJson, status);

    private static unsafe string? Read(byte* ptr, nuint len)
        => ptr == null ? null : Encoding.UTF8.GetString(ptr, checked((int)len));
}

/// <summary>Thrown when a JSONLogic rule (or data / config JSON) fails to parse.</summary>
public sealed class ParseException : DatalogicException
{
    internal ParseException(
        string message,
        string? errorType,
        string? @operator,
        string? pathJson,
        EvaluationStatus status = EvaluationStatus.ParseError)
        : base(message, errorType, @operator, pathJson, status) { }
}

/// <summary>Thrown when rule evaluation fails (runtime error, thrown, NaN, type mismatch, …).</summary>
public sealed class EvaluateException : DatalogicException
{
    internal EvaluateException(
        string message,
        string? errorType,
        string? @operator,
        string? pathJson,
        EvaluationStatus status = EvaluationStatus.EvaluationError)
        : base(message, errorType, @operator, pathJson, status) { }
}
