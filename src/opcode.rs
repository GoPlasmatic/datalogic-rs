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
//! Operators are grouped by functionality for code organization:
//!
//! - Variable Access (0-1, 57): `var`, `val`, `exists`
//! - Comparison (2-9): `==`, `===`, `!=`, `!==`, `>`, `>=`, `<`, `<=`
//! - Logical (10-13): `!`, `!!`, `and`, `or`
//! - Control Flow (14-15, 56, 59): `if`, `?:`, `??`, `switch`/`match`
//! - Arithmetic (16-22, 49-51): `+`, `-`, `*`, `/`, `%`, `max`, `min`, `abs`, `ceil`, `floor`
//! - String (23-25, 38-43, 53): `cat`, `substr`, `in`, `starts_with`, etc.
//! - Array (26-32, 54-55): `merge`, `filter`, `map`, `reduce`, `all`, `some`, `none`, `sort`, `slice`
//! - DateTime (44-48, 58): `datetime`, `timestamp`, `parse_date`, `format_date`, `date_diff`, `now`
//! - Error Handling (35-36): `try`, `throw`
//! - Type/Missing (33-34, 37): `missing`, `missing_some`, `type`
//! - Special (52): `preserve`
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
    // === Variable & Data Access ===
    Var = 0,
    Val = 1,
    Exists = 57,

    // === Comparison Operators ===
    Equals = 2,
    StrictEquals = 3,
    NotEquals = 4,
    StrictNotEquals = 5,
    GreaterThan = 6,
    GreaterThanEqual = 7,
    LessThan = 8,
    LessThanEqual = 9,

    // === Logical Operators ===
    Not = 10,
    DoubleNot = 11,
    And = 12,
    Or = 13,

    // === Control Flow ===
    If = 14,
    Ternary = 15,
    Coalesce = 56,
    Switch = 59,

    // === Arithmetic Operators ===
    Add = 16,
    Subtract = 17,
    Multiply = 18,
    Divide = 19,
    Modulo = 20,
    Max = 21,
    Min = 22,
    Abs = 49,
    Ceil = 50,
    Floor = 51,

    // === String Operations ===
    Cat = 23,
    Substr = 24,
    In = 25,
    Length = 53,
    StartsWith = 38,
    EndsWith = 39,
    Upper = 40,
    Lower = 41,
    Trim = 42,
    Split = 43,

    // === Array Operations ===
    Merge = 26,
    Filter = 27,
    Map = 28,
    Reduce = 29,
    All = 30,
    Some = 31,
    None = 32,
    Sort = 54,
    Slice = 55,

    // === DateTime Operations ===
    Datetime = 44,
    Timestamp = 45,
    ParseDate = 46,
    FormatDate = 47,
    DateDiff = 48,
    Now = 58,

    // === Error Handling ===
    Try = 35,
    Throw = 36,

    // === Type Operations ===
    Type = 37,

    // === Missing Value Handling ===
    Missing = 33,
    MissingSome = 34,

    // === Special Operations ===
    Preserve = 52,
}

impl FromStr for OpCode {
    type Err = ();

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "var" => Ok(OpCode::Var),
            "val" => Ok(OpCode::Val),
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
            "try" => Ok(OpCode::Try),
            "throw" => Ok(OpCode::Throw),
            "type" => Ok(OpCode::Type),
            "starts_with" => Ok(OpCode::StartsWith),
            "ends_with" => Ok(OpCode::EndsWith),
            "upper" => Ok(OpCode::Upper),
            "lower" => Ok(OpCode::Lower),
            "trim" => Ok(OpCode::Trim),
            "split" => Ok(OpCode::Split),
            "datetime" => Ok(OpCode::Datetime),
            "timestamp" => Ok(OpCode::Timestamp),
            "parse_date" => Ok(OpCode::ParseDate),
            "format_date" => Ok(OpCode::FormatDate),
            "date_diff" => Ok(OpCode::DateDiff),
            "now" => Ok(OpCode::Now),
            "abs" => Ok(OpCode::Abs),
            "ceil" => Ok(OpCode::Ceil),
            "floor" => Ok(OpCode::Floor),
            "preserve" => Ok(OpCode::Preserve),
            "length" => Ok(OpCode::Length),
            "sort" => Ok(OpCode::Sort),
            "slice" => Ok(OpCode::Slice),
            "??" => Ok(OpCode::Coalesce),
            "exists" => Ok(OpCode::Exists),
            "switch" | "match" => Ok(OpCode::Switch),
            _ => Err(()),
        }
    }
}

impl OpCode {
    /// Total number of built-in operators
    pub const COUNT: usize = 60;

    /// Convert OpCode back to string (for debugging/display)
    pub fn as_str(&self) -> &'static str {
        match self {
            OpCode::Var => "var",
            OpCode::Val => "val",
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
            OpCode::Try => "try",
            OpCode::Throw => "throw",
            OpCode::Type => "type",
            OpCode::StartsWith => "starts_with",
            OpCode::EndsWith => "ends_with",
            OpCode::Upper => "upper",
            OpCode::Lower => "lower",
            OpCode::Trim => "trim",
            OpCode::Split => "split",
            OpCode::Datetime => "datetime",
            OpCode::Timestamp => "timestamp",
            OpCode::ParseDate => "parse_date",
            OpCode::FormatDate => "format_date",
            OpCode::DateDiff => "date_diff",
            OpCode::Now => "now",
            OpCode::Abs => "abs",
            OpCode::Ceil => "ceil",
            OpCode::Floor => "floor",
            OpCode::Preserve => "preserve",
            OpCode::Length => "length",
            OpCode::Sort => "sort",
            OpCode::Slice => "slice",
            OpCode::Coalesce => "??",
            OpCode::Exists => "exists",
            OpCode::Switch => "switch",
        }
    }

    /// Direct evaluation method - no boxing, no vtables, no array lookups
    #[inline]
    pub fn evaluate_direct(
        &self,
        args: &[crate::CompiledNode],
        context: &mut crate::ContextStack,
        engine: &crate::DataLogic,
    ) -> crate::Result<serde_json::Value> {
        use crate::operators::{
            arithmetic, array, comparison, control, datetime, logical, math, missing, preserve,
            string, throw, try_op, type_op, variable,
        };

        match self {
            // Variable access operators - direct function calls
            OpCode::Var => variable::evaluate_var(args, context, engine),
            OpCode::Val => variable::evaluate_val(args, context, engine),
            OpCode::Exists => variable::evaluate_exists(args, context, engine),

            // Comparison operators - direct function calls
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

            // Logical operators - direct function calls
            OpCode::Not => logical::evaluate_not(args, context, engine),
            OpCode::DoubleNot => logical::evaluate_double_not(args, context, engine),
            OpCode::And => logical::evaluate_and(args, context, engine),
            OpCode::Or => logical::evaluate_or(args, context, engine),

            // Control flow - direct function calls
            OpCode::If => control::evaluate_if(args, context, engine),
            OpCode::Ternary => control::evaluate_ternary(args, context, engine),
            OpCode::Coalesce => control::evaluate_coalesce(args, context, engine),
            OpCode::Switch => control::evaluate_switch(args, context, engine),

            // Arithmetic operators - direct function calls
            OpCode::Add => arithmetic::evaluate_add(args, context, engine),
            OpCode::Subtract => arithmetic::evaluate_subtract(args, context, engine),
            OpCode::Multiply => arithmetic::evaluate_multiply(args, context, engine),
            OpCode::Divide => arithmetic::evaluate_divide(args, context, engine),
            OpCode::Modulo => arithmetic::evaluate_modulo(args, context, engine),
            OpCode::Max => arithmetic::evaluate_max(args, context, engine),
            OpCode::Min => arithmetic::evaluate_min(args, context, engine),
            OpCode::Abs => math::evaluate_abs(args, context, engine),
            OpCode::Ceil => math::evaluate_ceil(args, context, engine),
            OpCode::Floor => math::evaluate_floor(args, context, engine),

            // String operators - direct function calls
            OpCode::Cat => string::evaluate_cat(args, context, engine),
            OpCode::Substr => string::evaluate_substr(args, context, engine),
            OpCode::In => string::evaluate_in(args, context, engine),
            OpCode::Length => string::evaluate_length(args, context, engine),
            OpCode::StartsWith => string::evaluate_starts_with(args, context, engine),
            OpCode::EndsWith => string::evaluate_ends_with(args, context, engine),
            OpCode::Upper => string::evaluate_upper(args, context, engine),
            OpCode::Lower => string::evaluate_lower(args, context, engine),
            OpCode::Trim => string::evaluate_trim(args, context, engine),
            OpCode::Split => string::evaluate_split(args, context, engine),

            // Array operators - direct function calls
            OpCode::Merge => array::evaluate_merge(args, context, engine),
            OpCode::Filter => array::evaluate_filter(args, context, engine),
            OpCode::Map => array::evaluate_map(args, context, engine),
            OpCode::Reduce => array::evaluate_reduce(args, context, engine),
            OpCode::All => array::evaluate_all(args, context, engine),
            OpCode::Some => array::evaluate_some(args, context, engine),
            OpCode::None => array::evaluate_none(args, context, engine),
            OpCode::Sort => array::evaluate_sort(args, context, engine),
            OpCode::Slice => array::evaluate_slice(args, context, engine),

            // Special operators - direct function calls
            OpCode::Missing => missing::evaluate_missing(args, context, engine),
            OpCode::MissingSome => missing::evaluate_missing_some(args, context, engine),
            OpCode::Try => try_op::evaluate_try(args, context, engine),
            OpCode::Throw => throw::evaluate_throw(args, context, engine),
            OpCode::Type => type_op::evaluate_type(args, context, engine),
            OpCode::Preserve => preserve::evaluate_preserve(args, context, engine),

            // DateTime operators - direct function calls
            OpCode::Datetime => datetime::evaluate_datetime(args, context, engine),
            OpCode::Timestamp => datetime::evaluate_timestamp(args, context, engine),
            OpCode::ParseDate => datetime::evaluate_parse_date(args, context, engine),
            OpCode::FormatDate => datetime::evaluate_format_date(args, context, engine),
            OpCode::DateDiff => datetime::evaluate_date_diff(args, context, engine),
            OpCode::Now => datetime::evaluate_now(args, context, engine),
        }
    }

    /// Traced evaluation method - records steps for debugging.
    ///
    /// This method dispatches to traced versions of operators that need special
    /// handling (iteration and short-circuit operators), while regular operators
    /// use the standard evaluation with child tracing.
    #[inline]
    pub fn evaluate_traced(
        &self,
        args: &[crate::CompiledNode],
        context: &mut crate::ContextStack,
        engine: &crate::DataLogic,
        collector: &mut crate::trace::TraceCollector,
        node_id_map: &std::collections::HashMap<usize, u32>,
    ) -> crate::Result<serde_json::Value> {
        use crate::operators::{array, control, logical, throw, try_op};

        match self {
            // Iteration operators - need traced versions
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

            // Short-circuit logical operators - need traced versions
            OpCode::And => {
                logical::evaluate_and_traced(args, context, engine, collector, node_id_map)
            }
            OpCode::Or => {
                logical::evaluate_or_traced(args, context, engine, collector, node_id_map)
            }

            // Control flow operators - need traced versions
            OpCode::If => {
                control::evaluate_if_traced(args, context, engine, collector, node_id_map)
            }
            OpCode::Ternary => {
                control::evaluate_ternary_traced(args, context, engine, collector, node_id_map)
            }
            OpCode::Coalesce => {
                control::evaluate_coalesce_traced(args, context, engine, collector, node_id_map)
            }
            OpCode::Switch => {
                control::evaluate_switch_traced(args, context, engine, collector, node_id_map)
            }

            // Error handling operators - need traced versions
            OpCode::Try => {
                try_op::evaluate_try_traced(args, context, engine, collector, node_id_map)
            }
            OpCode::Throw => {
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
