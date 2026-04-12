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

    /// Direct evaluation method - no boxing, no vtables, no array lookups
    pub fn evaluate_direct(
        &self,
        args: &[crate::CompiledNode],
        context: &mut crate::ContextStack,
        engine: &crate::DataLogic,
    ) -> crate::Result<serde_json::Value> {
        use crate::operators::{arithmetic, array, comparison, control, logical, missing, string, variable};

        match self {
            // Core: Variable access
            OpCode::Var => variable::evaluate_var(args, context, engine),

            // Core: Comparison operators
            OpCode::Equals => comparison::evaluate_equals(args, context, engine),
            OpCode::StrictEquals => comparison::evaluate_strict_equals(args, context, engine),
            OpCode::NotEquals => comparison::evaluate_not_equals(args, context, engine),
            OpCode::StrictNotEquals => {
                comparison::evaluate_strict_not_equals(args, context, engine)
            }
            OpCode::GreaterThan => comparison::evaluate_greater_than(args, context, engine),
            OpCode::GreaterThanEqual => {
                comparison::evaluate_greater_than_equal(args, context, engine)
            }
            OpCode::LessThan => comparison::evaluate_less_than(args, context, engine),
            OpCode::LessThanEqual => comparison::evaluate_less_than_equal(args, context, engine),

            // Core: Logical operators
            OpCode::Not => logical::evaluate_not(args, context, engine),
            OpCode::DoubleNot => logical::evaluate_double_not(args, context, engine),
            OpCode::And => logical::evaluate_and(args, context, engine),
            OpCode::Or => logical::evaluate_or(args, context, engine),

            // Core: Control flow
            OpCode::If => control::evaluate_if(args, context, engine),
            OpCode::Ternary => control::evaluate_ternary(args, context, engine),

            // Core: Arithmetic operators
            OpCode::Add => arithmetic::evaluate_add(args, context, engine),
            OpCode::Subtract => arithmetic::evaluate_subtract(args, context, engine),
            OpCode::Multiply => arithmetic::evaluate_multiply(args, context, engine),
            OpCode::Divide => arithmetic::evaluate_divide(args, context, engine),
            OpCode::Modulo => arithmetic::evaluate_modulo(args, context, engine),
            OpCode::Max => arithmetic::evaluate_max(args, context, engine),
            OpCode::Min => arithmetic::evaluate_min(args, context, engine),

            // Core: String operators
            OpCode::Cat => string::evaluate_cat(args, context, engine),
            OpCode::Substr => string::evaluate_substr(args, context, engine),
            OpCode::In => string::evaluate_in(args, context, engine),

            // Core: Array operators
            OpCode::Merge => array::evaluate_merge(args, context, engine),
            OpCode::Filter => array::evaluate_filter(args, context, engine),
            OpCode::Map => array::evaluate_map(args, context, engine),
            OpCode::Reduce => array::evaluate_reduce(args, context, engine),
            OpCode::All => array::evaluate_all(args, context, engine),
            OpCode::Some => array::evaluate_some(args, context, engine),
            OpCode::None => array::evaluate_none(args, context, engine),

            // Core: Missing
            OpCode::Missing => missing::evaluate_missing(args, context, engine),
            OpCode::MissingSome => missing::evaluate_missing_some(args, context, engine),

            // preserve
            #[cfg(feature = "preserve")]
            OpCode::Preserve => {
                use crate::operators::preserve;
                preserve::evaluate_preserve(args, context, engine)
            }

            // datetime
            #[cfg(feature = "datetime")]
            OpCode::Datetime => {
                use crate::operators::datetime;
                datetime::evaluate_datetime(args, context, engine)
            }
            #[cfg(feature = "datetime")]
            OpCode::Timestamp => {
                use crate::operators::datetime;
                datetime::evaluate_timestamp(args, context, engine)
            }
            #[cfg(feature = "datetime")]
            OpCode::ParseDate => {
                use crate::operators::datetime;
                datetime::evaluate_parse_date(args, context, engine)
            }
            #[cfg(feature = "datetime")]
            OpCode::FormatDate => {
                use crate::operators::datetime;
                datetime::evaluate_format_date(args, context, engine)
            }
            #[cfg(feature = "datetime")]
            OpCode::DateDiff => {
                use crate::operators::datetime;
                datetime::evaluate_date_diff(args, context, engine)
            }
            #[cfg(feature = "datetime")]
            OpCode::Now => {
                use crate::operators::datetime;
                datetime::evaluate_now(args, context, engine)
            }

            // ext-string
            #[cfg(feature = "ext-string")]
            OpCode::Length => string::evaluate_length(args, context, engine),
            #[cfg(feature = "ext-string")]
            OpCode::StartsWith => string::evaluate_starts_with(args, context, engine),
            #[cfg(feature = "ext-string")]
            OpCode::EndsWith => string::evaluate_ends_with(args, context, engine),
            #[cfg(feature = "ext-string")]
            OpCode::Upper => string::evaluate_upper(args, context, engine),
            #[cfg(feature = "ext-string")]
            OpCode::Lower => string::evaluate_lower(args, context, engine),
            #[cfg(feature = "ext-string")]
            OpCode::Trim => string::evaluate_trim(args, context, engine),
            #[cfg(feature = "ext-string")]
            OpCode::Split => string::evaluate_split(args, context, engine),

            // ext-array
            #[cfg(feature = "ext-array")]
            OpCode::Sort => array::evaluate_sort(args, context, engine),
            #[cfg(feature = "ext-array")]
            OpCode::Slice => array::evaluate_slice(args, context, engine),

            // ext-control
            #[cfg(feature = "ext-control")]
            OpCode::Val => variable::evaluate_val(args, context, engine),
            #[cfg(feature = "ext-control")]
            OpCode::Exists => variable::evaluate_exists(args, context, engine),
            #[cfg(feature = "ext-control")]
            OpCode::Coalesce => control::evaluate_coalesce(args, context, engine),
            #[cfg(feature = "ext-control")]
            OpCode::Switch => control::evaluate_switch(args, context, engine),
            #[cfg(feature = "ext-control")]
            OpCode::Type => {
                use crate::operators::type_op;
                type_op::evaluate_type(args, context, engine)
            }

            // error-handling
            #[cfg(feature = "error-handling")]
            OpCode::Try => {
                use crate::operators::try_op;
                try_op::evaluate_try(args, context, engine)
            }
            #[cfg(feature = "error-handling")]
            OpCode::Throw => {
                use crate::operators::throw;
                throw::evaluate_throw(args, context, engine)
            }

            // ext-math
            #[cfg(feature = "ext-math")]
            OpCode::Abs => {
                use crate::operators::math;
                math::evaluate_abs(args, context, engine)
            }
            #[cfg(feature = "ext-math")]
            OpCode::Ceil => {
                use crate::operators::math;
                math::evaluate_ceil(args, context, engine)
            }
            #[cfg(feature = "ext-math")]
            OpCode::Floor => {
                use crate::operators::math;
                math::evaluate_floor(args, context, engine)
            }
        }
    }

    /// Traced evaluation method - records steps for debugging.
    ///
    /// This method dispatches to traced versions of operators that need special
    /// handling (iteration and short-circuit operators), while regular operators
    /// use the standard evaluation with child tracing.
    #[cfg(feature = "trace")]
    pub fn evaluate_traced(
        &self,
        args: &[crate::CompiledNode],
        context: &mut crate::ContextStack,
        engine: &crate::DataLogic,
        collector: &mut crate::trace::TraceCollector,
        node_id_map: &std::collections::HashMap<usize, u32>,
    ) -> crate::Result<serde_json::Value> {
        use crate::operators::{array, control, logical};

        match self {
            // Core: Iteration operators - need traced versions
            OpCode::Map => {
                array::evaluate_map_traced(args, context, engine, collector, node_id_map)
            }
            OpCode::Filter => {
                array::evaluate_filter_traced(args, context, engine, collector, node_id_map)
            }
            OpCode::Reduce => {
                array::evaluate_reduce_traced(args, context, engine, collector, node_id_map)
            }
            OpCode::All => {
                array::evaluate_all_traced(args, context, engine, collector, node_id_map)
            }
            OpCode::Some => {
                array::evaluate_some_traced(args, context, engine, collector, node_id_map)
            }
            OpCode::None => {
                array::evaluate_none_traced(args, context, engine, collector, node_id_map)
            }

            // Core: Short-circuit logical operators - need traced versions
            OpCode::And => {
                logical::evaluate_and_traced(args, context, engine, collector, node_id_map)
            }
            OpCode::Or => {
                logical::evaluate_or_traced(args, context, engine, collector, node_id_map)
            }

            // Core: Control flow operators - need traced versions
            OpCode::If => {
                control::evaluate_if_traced(args, context, engine, collector, node_id_map)
            }
            OpCode::Ternary => {
                control::evaluate_ternary_traced(args, context, engine, collector, node_id_map)
            }

            // ext-control: traced versions
            #[cfg(feature = "ext-control")]
            OpCode::Coalesce => {
                control::evaluate_coalesce_traced(args, context, engine, collector, node_id_map)
            }
            #[cfg(feature = "ext-control")]
            OpCode::Switch => {
                control::evaluate_switch_traced(args, context, engine, collector, node_id_map)
            }

            // error-handling: traced versions
            #[cfg(feature = "error-handling")]
            OpCode::Try => {
                use crate::operators::try_op;
                try_op::evaluate_try_traced(args, context, engine, collector, node_id_map)
            }
            #[cfg(feature = "error-handling")]
            OpCode::Throw => {
                use crate::operators::throw;
                throw::evaluate_throw_traced(args, context, engine, collector, node_id_map)
            }

            // All other operators - evaluate children with tracing, then apply operator
            _ => {
                // Evaluate all arguments with tracing
                let mut evaluated_args: Vec<serde_json::Value> = Vec::with_capacity(args.len());
                for arg in args {
                    let value =
                        engine.evaluate_node_traced(arg, context, collector, node_id_map)?;
                    evaluated_args.push(value);
                }

                // Create temporary Value nodes for the direct evaluation
                let value_nodes: Vec<crate::CompiledNode> = evaluated_args
                    .into_iter()
                    .map(|v| crate::CompiledNode::Value { value: v })
                    .collect();

                // Evaluate the operator with pre-evaluated arguments
                self.evaluate_direct(&value_nodes, context, engine)
            }
        }
    }
}
