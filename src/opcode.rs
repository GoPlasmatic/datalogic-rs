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
//!   - Variable Access: `var`
//!   - Comparison: `==`, `===`, `!=`, `!==`, `>`, `>=`, `<`, `<=`
//!   - Logical: `!`, `!!`, `and`, `or`
//!   - Control Flow: `if`, `?:`
//!   - Arithmetic: `+`, `-`, `*`, `/`, `%`, `max`, `min`
//!   - String: `cat`, `substr`, `in`
//!   - Array: `merge`, `filter`, `map`, `reduce`, `all`, `some`, `none`
//!   - Missing: `missing`, `missing_some`
//! - **preserve**: `preserve`
//! - **datetime**: `datetime`, `timestamp`, `parse_date`, `format_date`, `date_diff`, `now`
//! - **ext-string**: `length`, `starts_with`, `ends_with`, `upper`, `lower`, `trim`, `split`
//! - **ext-array**: `sort`, `slice`
//! - **ext-control**: `val`, `exists`, `??`, `switch`/`match`, `type`
//! - **error-handling**: `try`, `throw`
//! - **ext-math**: `abs`, `ceil`, `floor`
//!
//! # Adding New Operators
//!
//! 1. Add a new variant to the `OpCode` enum
//! 2. Add string mapping in `FromStr` implementation
//! 3. Add reverse mapping in `as_str()` method
//! 4. Add dispatch case in `evaluate_direct()`
//! 5. Implement the operator function in the appropriate `operators/` module

use std::str::FromStr;

/// OpCode enum for fast built-in operator lookup
#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OpCode {
    // === Core: Variable Access ===
    Var = 0,

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
    DoubleNot = 11,
    And = 12,
    Or = 13,

    // === Core: Control Flow ===
    If = 14,
    Ternary = 15,

    // === Core: Arithmetic Operators ===
    Add = 16,
    Subtract = 17,
    Multiply = 18,
    Divide = 19,
    Modulo = 20,
    Max = 21,
    Min = 22,

    // === Core: String Operations ===
    Cat = 23,
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
    Val = 1,
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

impl FromStr for OpCode {
    type Err = ();

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            // Core
            "var" => Ok(OpCode::Var),
            "==" => Ok(OpCode::Equals),
            "===" => Ok(OpCode::StrictEquals),
            "!=" => Ok(OpCode::NotEquals),
            "!==" => Ok(OpCode::StrictNotEquals),
            ">" => Ok(OpCode::GreaterThan),
            ">=" => Ok(OpCode::GreaterThanEqual),
            "<" => Ok(OpCode::LessThan),
            "<=" => Ok(OpCode::LessThanEqual),
            "!" => Ok(OpCode::Not),
            "!!" => Ok(OpCode::DoubleNot),
            "and" => Ok(OpCode::And),
            "or" => Ok(OpCode::Or),
            "if" => Ok(OpCode::If),
            "?:" => Ok(OpCode::Ternary),
            "+" => Ok(OpCode::Add),
            "-" => Ok(OpCode::Subtract),
            "*" => Ok(OpCode::Multiply),
            "/" => Ok(OpCode::Divide),
            "%" => Ok(OpCode::Modulo),
            "max" => Ok(OpCode::Max),
            "min" => Ok(OpCode::Min),
            "cat" => Ok(OpCode::Cat),
            "substr" => Ok(OpCode::Substr),
            "in" => Ok(OpCode::In),
            "merge" => Ok(OpCode::Merge),
            "filter" => Ok(OpCode::Filter),
            "map" => Ok(OpCode::Map),
            "reduce" => Ok(OpCode::Reduce),
            "all" => Ok(OpCode::All),
            "some" => Ok(OpCode::Some),
            "none" => Ok(OpCode::None),
            "missing" => Ok(OpCode::Missing),
            "missing_some" => Ok(OpCode::MissingSome),

            // preserve
            #[cfg(feature = "preserve")]
            "preserve" => Ok(OpCode::Preserve),

            // datetime
            #[cfg(feature = "datetime")]
            "datetime" => Ok(OpCode::Datetime),
            #[cfg(feature = "datetime")]
            "timestamp" => Ok(OpCode::Timestamp),
            #[cfg(feature = "datetime")]
            "parse_date" => Ok(OpCode::ParseDate),
            #[cfg(feature = "datetime")]
            "format_date" => Ok(OpCode::FormatDate),
            #[cfg(feature = "datetime")]
            "date_diff" => Ok(OpCode::DateDiff),
            #[cfg(feature = "datetime")]
            "now" => Ok(OpCode::Now),

            // ext-string
            #[cfg(feature = "ext-string")]
            "length" => Ok(OpCode::Length),
            #[cfg(feature = "ext-string")]
            "starts_with" => Ok(OpCode::StartsWith),
            #[cfg(feature = "ext-string")]
            "ends_with" => Ok(OpCode::EndsWith),
            #[cfg(feature = "ext-string")]
            "upper" => Ok(OpCode::Upper),
            #[cfg(feature = "ext-string")]
            "lower" => Ok(OpCode::Lower),
            #[cfg(feature = "ext-string")]
            "trim" => Ok(OpCode::Trim),
            #[cfg(feature = "ext-string")]
            "split" => Ok(OpCode::Split),

            // ext-array
            #[cfg(feature = "ext-array")]
            "sort" => Ok(OpCode::Sort),
            #[cfg(feature = "ext-array")]
            "slice" => Ok(OpCode::Slice),

            // ext-control
            #[cfg(feature = "ext-control")]
            "val" => Ok(OpCode::Val),
            #[cfg(feature = "ext-control")]
            "exists" => Ok(OpCode::Exists),
            #[cfg(feature = "ext-control")]
            "??" => Ok(OpCode::Coalesce),
            #[cfg(feature = "ext-control")]
            "switch" | "match" => Ok(OpCode::Switch),
            #[cfg(feature = "ext-control")]
            "type" => Ok(OpCode::Type),

            // error-handling
            #[cfg(feature = "error-handling")]
            "try" => Ok(OpCode::Try),
            #[cfg(feature = "error-handling")]
            "throw" => Ok(OpCode::Throw),

            // ext-math
            #[cfg(feature = "ext-math")]
            "abs" => Ok(OpCode::Abs),
            #[cfg(feature = "ext-math")]
            "ceil" => Ok(OpCode::Ceil),
            #[cfg(feature = "ext-math")]
            "floor" => Ok(OpCode::Floor),

            _ => Err(()),
        }
    }
}

impl OpCode {
    /// Convert OpCode back to string (for debugging/display)
    pub fn as_str(&self) -> &'static str {
        match self {
            // Core
            OpCode::Var => "var",
            OpCode::Equals => "==",
            OpCode::StrictEquals => "===",
            OpCode::NotEquals => "!=",
            OpCode::StrictNotEquals => "!==",
            OpCode::GreaterThan => ">",
            OpCode::GreaterThanEqual => ">=",
            OpCode::LessThan => "<",
            OpCode::LessThanEqual => "<=",
            OpCode::Not => "!",
            OpCode::DoubleNot => "!!",
            OpCode::And => "and",
            OpCode::Or => "or",
            OpCode::If => "if",
            OpCode::Ternary => "?:",
            OpCode::Add => "+",
            OpCode::Subtract => "-",
            OpCode::Multiply => "*",
            OpCode::Divide => "/",
            OpCode::Modulo => "%",
            OpCode::Max => "max",
            OpCode::Min => "min",
            OpCode::Cat => "cat",
            OpCode::Substr => "substr",
            OpCode::In => "in",
            OpCode::Merge => "merge",
            OpCode::Filter => "filter",
            OpCode::Map => "map",
            OpCode::Reduce => "reduce",
            OpCode::All => "all",
            OpCode::Some => "some",
            OpCode::None => "none",
            OpCode::Missing => "missing",
            OpCode::MissingSome => "missing_some",

            // preserve
            #[cfg(feature = "preserve")]
            OpCode::Preserve => "preserve",

            // datetime
            #[cfg(feature = "datetime")]
            OpCode::Datetime => "datetime",
            #[cfg(feature = "datetime")]
            OpCode::Timestamp => "timestamp",
            #[cfg(feature = "datetime")]
            OpCode::ParseDate => "parse_date",
            #[cfg(feature = "datetime")]
            OpCode::FormatDate => "format_date",
            #[cfg(feature = "datetime")]
            OpCode::DateDiff => "date_diff",
            #[cfg(feature = "datetime")]
            OpCode::Now => "now",

            // ext-string
            #[cfg(feature = "ext-string")]
            OpCode::Length => "length",
            #[cfg(feature = "ext-string")]
            OpCode::StartsWith => "starts_with",
            #[cfg(feature = "ext-string")]
            OpCode::EndsWith => "ends_with",
            #[cfg(feature = "ext-string")]
            OpCode::Upper => "upper",
            #[cfg(feature = "ext-string")]
            OpCode::Lower => "lower",
            #[cfg(feature = "ext-string")]
            OpCode::Trim => "trim",
            #[cfg(feature = "ext-string")]
            OpCode::Split => "split",

            // ext-array
            #[cfg(feature = "ext-array")]
            OpCode::Sort => "sort",
            #[cfg(feature = "ext-array")]
            OpCode::Slice => "slice",

            // ext-control
            #[cfg(feature = "ext-control")]
            OpCode::Val => "val",
            #[cfg(feature = "ext-control")]
            OpCode::Exists => "exists",
            #[cfg(feature = "ext-control")]
            OpCode::Coalesce => "??",
            #[cfg(feature = "ext-control")]
            OpCode::Switch => "switch",
            #[cfg(feature = "ext-control")]
            OpCode::Type => "type",

            // error-handling
            #[cfg(feature = "error-handling")]
            OpCode::Try => "try",
            #[cfg(feature = "error-handling")]
            OpCode::Throw => "throw",

            // ext-math
            #[cfg(feature = "ext-math")]
            OpCode::Abs => "abs",
            #[cfg(feature = "ext-math")]
            OpCode::Ceil => "ceil",
            #[cfg(feature = "ext-math")]
            OpCode::Floor => "floor",
        }
    }
}
