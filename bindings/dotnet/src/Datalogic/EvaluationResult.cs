// SPDX-License-Identifier: Apache-2.0

namespace Goplasmatic.Datalogic;

/// <summary>
/// Coarse outcome of an evaluation, mirroring the engine's C ABI status
/// codes. Carried on every <see cref="DatalogicException"/> and on each
/// per-item <see cref="EvaluationResult"/> of the batch APIs; the
/// fine-grained engine tag (e.g. <c>"NaN"</c>, <c>"Thrown"</c>) stays
/// available alongside it.
/// </summary>
public enum EvaluationStatus
{
    /// <summary>Success.</summary>
    Ok = 0,
    /// <summary>Invalid argument — e.g. a rule compiled by a different engine than the session's.</summary>
    InvalidArgument = 1,
    /// <summary>Rule / data / config JSON failed to parse.</summary>
    ParseError = 2,
    /// <summary>Evaluation failed; the error tag carries the detail (<c>"Thrown"</c>, <c>"NaN"</c>, …).</summary>
    EvaluationError = 3,
    /// <summary>A typed evaluation succeeded but the result is not of the requested type.</summary>
    TypeMismatch = 4,
    /// <summary>An internal engine failure was caught at the native boundary.</summary>
    InternalError = 5,
}

/// <summary>
/// Per-item outcome of <see cref="Session.EvaluateBatch"/> and
/// <see cref="Session.EvaluateMany"/>: either the result JSON or the
/// item's error detail. Item failures never throw from the batch call
/// itself — inspect <see cref="IsSuccess"/> (or let <see cref="Value"/>
/// throw the mapped exception).
/// </summary>
public readonly struct EvaluationResult
{
    private readonly string? _json;

    private EvaluationResult(
        EvaluationStatus status,
        string? json,
        string? errorTag,
        string? errorMessage,
        string? errorOperator)
    {
        Status = status;
        _json = json;
        ErrorTag = errorTag;
        ErrorMessage = errorMessage;
        ErrorOperator = errorOperator;
    }

    internal static EvaluationResult Success(string json)
        => new(EvaluationStatus.Ok, json, null, null, null);

    internal static EvaluationResult Failure(
        EvaluationStatus status,
        string? errorTag,
        string errorMessage,
        string? errorOperator)
        => new(status, null, errorTag, errorMessage, errorOperator);

    /// <summary>The item's status (<see cref="EvaluationStatus.Ok"/> on success).</summary>
    public EvaluationStatus Status { get; }

    /// <summary>Whether the item evaluated successfully.</summary>
    public bool IsSuccess => Status == EvaluationStatus.Ok;

    /// <summary>The result as a JSON string, or <c>null</c> if the item failed.</summary>
    public string? Json => _json;

    /// <summary>Stable engine error tag (e.g. <c>"Thrown"</c>, <c>"NaN"</c>), or <c>null</c> on success.</summary>
    public string? ErrorTag { get; }

    /// <summary>Human-readable error message, or <c>null</c> on success.</summary>
    public string? ErrorMessage { get; }

    /// <summary>Outermost failing operator (e.g. <c>"+"</c>), or <c>null</c> when not operator-scoped or on success.</summary>
    public string? ErrorOperator { get; }

    /// <summary>
    /// The result JSON, throwing the mapped exception
    /// (<see cref="ParseException"/> / <see cref="EvaluateException"/>)
    /// if the item failed — for callers that treat any item failure as
    /// exceptional.
    /// </summary>
    public string Value
        => IsSuccess
            ? _json!
            : throw DatalogicException.Create(
                Status,
                ErrorTag,
                ErrorMessage ?? $"batch item failed ({ErrorTag ?? "unknown"})",
                ErrorOperator,
                pathJson: null);

    /// <inheritdoc />
    public override string ToString()
        => IsSuccess ? _json! : $"<{ErrorTag ?? Status.ToString()}: {ErrorMessage}>";
}
