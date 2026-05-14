// SPDX-License-Identifier: Apache-2.0

using System.Text.Json;

using Goplasmatic.Datalogic.Native;

namespace Goplasmatic.Datalogic;

/// <summary>
/// Base class for every exception thrown by this binding. Mirrors the
/// thread-local last-error block exposed by the C ABI
/// (`datalogic_last_error_*`).
/// </summary>
public class DatalogicException : Exception
{
    /// <summary>Stable error tag from the engine (e.g. "ParseError", "Thrown", "NaN").</summary>
    public string? ErrorType { get; }
    /// <summary>Outermost failing operator name (e.g. "+"), or null if not operator-scoped.</summary>
    public string? Operator { get; }
    /// <summary>Resolved root-to-leaf error path as a JSON array, or null if not available.</summary>
    public string? PathJson { get; }

    internal DatalogicException(string message, string? errorType, string? @operator, string? pathJson)
        : base(message)
    {
        ErrorType = errorType;
        Operator = @operator;
        PathJson = pathJson;
    }

    /// <summary>
    /// Construct the appropriate subclass by reading the C ABI's
    /// thread-local last-error state. Falls back to a generic
    /// <see cref="DatalogicException"/> if no error is set.
    /// </summary>
    internal static DatalogicException FromLastError(string fallback)
    {
        var msg = NativeMethods.BorrowUtf8String(NativeMethods.datalogic_last_error_message()) ?? fallback;
        var type = NativeMethods.BorrowUtf8String(NativeMethods.datalogic_last_error_type());
        var op = NativeMethods.BorrowUtf8String(NativeMethods.datalogic_last_error_operator());
        var path = NativeMethods.BorrowUtf8String(NativeMethods.datalogic_last_error_path_json());
        return type switch
        {
            "ParseError" => new ParseException(msg, type, op, path),
            _ => new EvaluateException(msg, type, op, path),
        };
    }
}

/// <summary>Thrown when a JSONLogic rule fails to parse.</summary>
public sealed class ParseException : DatalogicException
{
    internal ParseException(string message, string? errorType, string? @operator, string? pathJson)
        : base(message, errorType, @operator, pathJson) { }
}

/// <summary>Thrown when rule evaluation fails (runtime error, thrown, NaN, …).</summary>
public sealed class EvaluateException : DatalogicException
{
    internal EvaluateException(string message, string? errorType, string? @operator, string? pathJson)
        : base(message, errorType, @operator, pathJson) { }
}
