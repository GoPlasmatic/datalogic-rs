//! OpCode-based dispatch system for built-in operators.
//!
//! This module implements a high-performance dispatch mechanism using enum variants
//! instead of string matching or vtable lookups at runtime.
//!
//! # Performance Design
//!
//! The `OpCode` enum provides O(1) operator dispatch through:
//!
//! 1. **Compile-time resolution**: Operator strings are converted to `OpCode` variants
//!    during the compilation phase, not during evaluation
//! 2. **Direct dispatch**: The `evaluate_direct` method uses a match statement that
//!    compiles to an efficient jump table
//! 3. **No boxing or vtables**: Direct function calls without trait object overhead
//! 4. **Cache-friendly**: The `#[repr(u8)]` attribute ensures compact memory layout
//!
//! # Operator Categories
//!
//! Operators are grouped by functionality and feature-gated:
//!
//! - **Core** (always available):
//!   - Variable Access: `val` (canonical; `var` is accepted as input and
//!     normalized to `val` at compile time)
//!   - Comparison: `==`, `===`, `!=`, `!==`, `>`, `>=`, `<`, `<=`
//!   - Logical: `!`, `!!`, `and`, `or`
//!   - Control Flow: `if` (canonical; `?:` is accepted as input and normalized
//!     to `if` at compile time)
//!   - Arithmetic: `+`, `-`, `*`, `/`, `%`, `max`, `min`
//!   - String: `cat`, `substr`, `in`
//!   - Array: `merge`, `filter`, `map`, `reduce`, `all`, `some`, `none`
//!   - Missing: `missing`, `missing_some`
//! - **preserve**: `preserve`
//! - **datetime**: `datetime`, `timestamp`, `parse_date`, `format_date`, `date_diff`, `now`
//! - **ext-string**: `length`, `starts_with`, `ends_with`, `upper`, `lower`, `trim`, `split`
//! - **ext-array**: `sort`, `slice`
//! - **ext-control**: `exists`, `??`, `switch`/`match`, `type`
//! - **error-handling**: `try`, `throw`
//! - **ext-math**: `abs`, `ceil`, `floor`
//!
//! # Adding New Operators
//!
//! 1. Add a new variant to the [`OpCode`] enum
//! 2. Add an entry (canonical name first, then any aliases) to [`OPCODE_NAMES`]
//! 3. Add the dispatch arm in `src/engine/dispatch.rs`
//! 4. Implement the operator function in the appropriate `src/operators/` module

use std::str::FromStr;

/// OpCode enum for fast built-in operator lookup
#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OpCode {
    // === Core: Variable Access ===
    Val = 1,

    // === Core: Comparison Operators ===
    Equals = 2,
    StrictEquals = 3,
    NotEquals = 4,
    StrictNotEquals = 5,
    GreaterThan = 6,
    GreaterThanEqual = 7,
    LessThan = 8,
    LessThanEqual = 9,

    // === Core: Logical Operators ===
    Not = 10,
    BoolCast = 11,
    And = 12,
    Or = 13,

    // === Core: Control Flow ===
    If = 14,

    // === Core: Arithmetic Operators ===
    Add = 16,
    Subtract = 17,
    Multiply = 18,
    Divide = 19,
    Modulo = 20,
    Max = 21,
    Min = 22,

    // === Core: String Operations ===
    Concat = 23,
    Substr = 24,
    In = 25,

    // === Core: Array Operations ===
    Merge = 26,
    Filter = 27,
    Map = 28,
    Reduce = 29,
    All = 30,
    Some = 31,
    None = 32,

    // === Core: Missing Value Handling ===
    Missing = 33,
    MissingSome = 34,

    // === preserve ===
    #[cfg(feature = "preserve")]
    Preserve = 52,

    // === datetime ===
    #[cfg(feature = "datetime")]
    Datetime = 44,
    #[cfg(feature = "datetime")]
    Timestamp = 45,
    #[cfg(feature = "datetime")]
    ParseDate = 46,
    #[cfg(feature = "datetime")]
    FormatDate = 47,
    #[cfg(feature = "datetime")]
    DateDiff = 48,
    #[cfg(feature = "datetime")]
    Now = 58,

    // === ext-string ===
    #[cfg(feature = "ext-string")]
    Length = 53,
    #[cfg(feature = "ext-string")]
    StartsWith = 38,
    #[cfg(feature = "ext-string")]
    EndsWith = 39,
    #[cfg(feature = "ext-string")]
    Upper = 40,
    #[cfg(feature = "ext-string")]
    Lower = 41,
    #[cfg(feature = "ext-string")]
    Trim = 42,
    #[cfg(feature = "ext-string")]
    Split = 43,

    // === ext-array ===
    #[cfg(feature = "ext-array")]
    Sort = 54,
    #[cfg(feature = "ext-array")]
    Slice = 55,

    // === ext-control ===
    #[cfg(feature = "ext-control")]
    Exists = 57,
    #[cfg(feature = "ext-control")]
    Coalesce = 56,
    #[cfg(feature = "ext-control")]
    Switch = 59,
    #[cfg(feature = "ext-control")]
    Type = 37,

    // === error-handling ===
    #[cfg(feature = "error-handling")]
    Try = 35,
    #[cfg(feature = "error-handling")]
    Throw = 36,

    // === ext-math ===
    #[cfg(feature = "ext-math")]
    Abs = 49,
    #[cfg(feature = "ext-math")]
    Ceil = 50,
    #[cfg(feature = "ext-math")]
    Floor = 51,
}

/// Single source of truth for `(operator string, OpCode)` mappings.
///
/// The first entry per opcode is the canonical name returned by
/// [`OpCode::as_str`]; subsequent entries with the same opcode are accepted
/// as aliases by [`OpCode::from_str`]. Feature-gated entries follow the
/// same `#[cfg]` as their corresponding [`OpCode`] variant.
const OPCODE_NAMES: &[(&str, OpCode)] = &[
    // Core: variable access. `var` is accepted as a synonym of `val` —
    // both normalize to OpCode::Val. The compile pipeline (`try_specialised`)
    // dispatches the appropriate compile-time specialiser based on the source
    // operator name.
    ("val", OpCode::Val),
    ("var", OpCode::Val),
    // Core: comparison
    ("==", OpCode::Equals),
    ("===", OpCode::StrictEquals),
    ("!=", OpCode::NotEquals),
    ("!==", OpCode::StrictNotEquals),
    (">", OpCode::GreaterThan),
    (">=", OpCode::GreaterThanEqual),
    ("<", OpCode::LessThan),
    ("<=", OpCode::LessThanEqual),
    // Core: logical
    ("!", OpCode::Not),
    ("!!", OpCode::BoolCast),
    ("and", OpCode::And),
    ("or", OpCode::Or),
    // Core: control flow. `?:` is accepted as a synonym of `if` — both
    // normalize to OpCode::If. `evaluate_if` already handles the 3-arg case
    // identically to a ternary.
    ("if", OpCode::If),
    ("?:", OpCode::If),
    // Core: arithmetic
    ("+", OpCode::Add),
    ("-", OpCode::Subtract),
    ("*", OpCode::Multiply),
    ("/", OpCode::Divide),
    ("%", OpCode::Modulo),
    ("max", OpCode::Max),
    ("min", OpCode::Min),
    // Core: string
    ("cat", OpCode::Concat),
    ("substr", OpCode::Substr),
    ("in", OpCode::In),
    // Core: array
    ("merge", OpCode::Merge),
    ("filter", OpCode::Filter),
    ("map", OpCode::Map),
    ("reduce", OpCode::Reduce),
    ("all", OpCode::All),
    ("some", OpCode::Some),
    ("none", OpCode::None),
    // Core: missing
    ("missing", OpCode::Missing),
    ("missing_some", OpCode::MissingSome),
    // preserve
    #[cfg(feature = "preserve")]
    ("preserve", OpCode::Preserve),
    // datetime
    #[cfg(feature = "datetime")]
    ("datetime", OpCode::Datetime),
    #[cfg(feature = "datetime")]
    ("timestamp", OpCode::Timestamp),
    #[cfg(feature = "datetime")]
    ("parse_date", OpCode::ParseDate),
    #[cfg(feature = "datetime")]
    ("format_date", OpCode::FormatDate),
    #[cfg(feature = "datetime")]
    ("date_diff", OpCode::DateDiff),
    #[cfg(feature = "datetime")]
    ("now", OpCode::Now),
    // ext-string
    #[cfg(feature = "ext-string")]
    ("length", OpCode::Length),
    #[cfg(feature = "ext-string")]
    ("starts_with", OpCode::StartsWith),
    #[cfg(feature = "ext-string")]
    ("ends_with", OpCode::EndsWith),
    #[cfg(feature = "ext-string")]
    ("upper", OpCode::Upper),
    #[cfg(feature = "ext-string")]
    ("lower", OpCode::Lower),
    #[cfg(feature = "ext-string")]
    ("trim", OpCode::Trim),
    #[cfg(feature = "ext-string")]
    ("split", OpCode::Split),
    // ext-array
    #[cfg(feature = "ext-array")]
    ("sort", OpCode::Sort),
    #[cfg(feature = "ext-array")]
    ("slice", OpCode::Slice),
    // ext-control
    #[cfg(feature = "ext-control")]
    ("exists", OpCode::Exists),
    #[cfg(feature = "ext-control")]
    ("??", OpCode::Coalesce),
    #[cfg(feature = "ext-control")]
    ("switch", OpCode::Switch),
    #[cfg(feature = "ext-control")]
    ("match", OpCode::Switch),
    #[cfg(feature = "ext-control")]
    ("type", OpCode::Type),
    // error-handling
    #[cfg(feature = "error-handling")]
    ("try", OpCode::Try),
    #[cfg(feature = "error-handling")]
    ("throw", OpCode::Throw),
    // ext-math
    #[cfg(feature = "ext-math")]
    ("abs", OpCode::Abs),
    #[cfg(feature = "ext-math")]
    ("ceil", OpCode::Ceil),
    #[cfg(feature = "ext-math")]
    ("floor", OpCode::Floor),
];

impl FromStr for OpCode {
    type Err = ();

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        // Linear scan over OPCODE_NAMES. Compilation is cold (one-shot per
        // rule) and the table is small (~60 entries), so this is fine.
        for (name, op) in OPCODE_NAMES {
            if *name == s {
                return Ok(*op);
            }
        }
        Err(())
    }
}

impl OpCode {
    /// Convert OpCode back to its canonical string form (for debugging /
    /// display / serialization). Returns the first entry in [`OPCODE_NAMES`]
    /// whose opcode matches `self`.
    pub fn as_str(&self) -> &'static str {
        for (name, op) in OPCODE_NAMES {
            if *op == *self {
                return name;
            }
        }
        // Unreachable: OPCODE_NAMES has an entry for every variant compiled
        // in the current feature set. If you've added a variant without a
        // name entry, `from_str` will reject it too — fix the table.
        unreachable!("OPCODE_NAMES missing entry for {self:?}")
    }
}
