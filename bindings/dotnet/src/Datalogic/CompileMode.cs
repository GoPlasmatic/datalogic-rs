// SPDX-License-Identifier: Apache-2.0

namespace Goplasmatic.Datalogic;

/// <summary>How <see cref="Engine.CompileMode"/> and <see cref="Engine.Check"/> read a rule.</summary>
public enum CompileMode : uint
{
    /// <summary>The engine's own mode, as <see cref="Engine.Compile"/> reads it.</summary>
    Engine = 0,
    /// <summary>Outside templating mode: a multi-key object or an unknown operator is an error.</summary>
    Strict = 1,
    /// <summary>In templating mode: a multi-key object is an output template, an unknown key a field.</summary>
    Template = 2,
}

/// <summary>A metered evaluation's result and the operations it charged.</summary>
/// <param name="Value">Result JSON string.</param>
/// <param name="Ops">
/// Operations charged: one per dispatched node, one per item an iterator examined, plus what
/// operators charged for the data they moved.
/// </param>
public readonly record struct MeteredResult(string Value, ulong Ops);
