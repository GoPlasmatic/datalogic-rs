use std::str::FromStr;

/// OpCode enum for fast built-in operator lookup
#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OpCode {
    // Variable operators
    Var = 0,
    Val = 1,

    // Comparison operators
    Equals = 2,
    StrictEquals = 3,
    NotEquals = 4,
    StrictNotEquals = 5,
    GreaterThan = 6,
    GreaterThanEqual = 7,
    LessThan = 8,
    LessThanEqual = 9,

    // Logical operators
    Not = 10,
    DoubleNot = 11,
    And = 12,
    Or = 13,
    If = 14,
    Ternary = 15,

    // Arithmetic operators
    Add = 16,
    Subtract = 17,
    Multiply = 18,
    Divide = 19,
    Modulo = 20,
    Max = 21,
    Min = 22,

    // String operators
    Cat = 23,
    Substr = 24,
    In = 25,
    Length = 53,

    // Array operators
    Merge = 26,
    Filter = 27,
    Map = 28,
    Reduce = 29,
    All = 30,
    Some = 31,
    None = 32,

    // Missing operators
    Missing = 33,
    MissingSome = 34,

    // Error handling operators
    Try = 35,
    Throw = 36,

    // Type operator
    Type = 37,

    // String operators
    StartsWith = 38,
    EndsWith = 39,
    Upper = 40,
    Lower = 41,
    Trim = 42,
    Split = 43,

    // Datetime operators
    Datetime = 44,
    Timestamp = 45,
    ParseDate = 46,
    FormatDate = 47,
    DateDiff = 48,
    Now = 58,

    // Math operators
    Abs = 49,
    Ceil = 50,
    Floor = 51,

    // Utility operators
    Preserve = 52,
    Sort = 54,
    Slice = 55,
    Coalesce = 56,
    Exists = 57,
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
            _ => Err(()),
        }
    }
}

impl OpCode {
    /// Total number of built-in operators
    pub const COUNT: usize = 59;

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
        }
    }

    /// Direct evaluation method - no boxing, no vtables, no array lookups
    #[inline]
    pub fn evaluate_direct(
        &self,
        args: &[serde_json::Value],
        context: &mut crate::ContextStack,
        evaluator: &dyn crate::Evaluator,
    ) -> crate::Result<serde_json::Value> {
        use crate::operators::{
            abs, arithmetic, array, ceil, comparison, control, datetime, floor, logical, missing,
            preserve, string, string_ops, throw, try_op, type_op, variable,
        };

        match self {
            // Variable access operators - direct function calls
            OpCode::Var => variable::evaluate_var(args, context, evaluator),
            OpCode::Val => variable::evaluate_val(args, context, evaluator),
            OpCode::Exists => variable::evaluate_exists(args, context, evaluator),

            // Comparison operators - direct function calls
            OpCode::Equals => comparison::evaluate_equals(args, context, evaluator),
            OpCode::StrictEquals => comparison::evaluate_strict_equals(args, context, evaluator),
            OpCode::NotEquals => comparison::evaluate_not_equals(args, context, evaluator),
            OpCode::StrictNotEquals => {
                comparison::evaluate_strict_not_equals(args, context, evaluator)
            }
            OpCode::GreaterThan => comparison::evaluate_greater_than(args, context, evaluator),
            OpCode::GreaterThanEqual => {
                comparison::evaluate_greater_than_equal(args, context, evaluator)
            }
            OpCode::LessThan => comparison::evaluate_less_than(args, context, evaluator),
            OpCode::LessThanEqual => comparison::evaluate_less_than_equal(args, context, evaluator),

            // Logical operators - direct function calls
            OpCode::Not => logical::evaluate_not(args, context, evaluator),
            OpCode::DoubleNot => logical::evaluate_double_not(args, context, evaluator),
            OpCode::And => logical::evaluate_and(args, context, evaluator),
            OpCode::Or => logical::evaluate_or(args, context, evaluator),

            // Control flow - direct function calls
            OpCode::If => control::evaluate_if(args, context, evaluator),
            OpCode::Ternary => control::evaluate_ternary(args, context, evaluator),
            OpCode::Coalesce => control::evaluate_coalesce(args, context, evaluator),

            // Arithmetic operators - direct function calls
            OpCode::Add => arithmetic::evaluate_add(args, context, evaluator),
            OpCode::Subtract => arithmetic::evaluate_subtract(args, context, evaluator),
            OpCode::Multiply => arithmetic::evaluate_multiply(args, context, evaluator),
            OpCode::Divide => arithmetic::evaluate_divide(args, context, evaluator),
            OpCode::Modulo => arithmetic::evaluate_modulo(args, context, evaluator),
            OpCode::Max => arithmetic::evaluate_max(args, context, evaluator),
            OpCode::Min => arithmetic::evaluate_min(args, context, evaluator),
            OpCode::Abs => abs::evaluate_abs(args, context, evaluator),
            OpCode::Ceil => ceil::evaluate_ceil(args, context, evaluator),
            OpCode::Floor => floor::evaluate_floor(args, context, evaluator),

            // String operators - direct function calls
            OpCode::Cat => string::evaluate_cat(args, context, evaluator),
            OpCode::Substr => string::evaluate_substr(args, context, evaluator),
            OpCode::In => string::evaluate_in(args, context, evaluator),
            OpCode::Length => string::evaluate_length(args, context, evaluator),
            OpCode::StartsWith => string_ops::evaluate_starts_with(args, context, evaluator),
            OpCode::EndsWith => string_ops::evaluate_ends_with(args, context, evaluator),
            OpCode::Upper => string_ops::evaluate_upper(args, context, evaluator),
            OpCode::Lower => string_ops::evaluate_lower(args, context, evaluator),
            OpCode::Trim => string_ops::evaluate_trim(args, context, evaluator),
            OpCode::Split => string_ops::evaluate_split(args, context, evaluator),

            // Array operators - direct function calls
            OpCode::Merge => array::evaluate_merge(args, context, evaluator),
            OpCode::Filter => array::evaluate_filter(args, context, evaluator),
            OpCode::Map => array::evaluate_map(args, context, evaluator),
            OpCode::Reduce => array::evaluate_reduce(args, context, evaluator),
            OpCode::All => array::evaluate_all(args, context, evaluator),
            OpCode::Some => array::evaluate_some(args, context, evaluator),
            OpCode::None => array::evaluate_none(args, context, evaluator),
            OpCode::Sort => array::evaluate_sort(args, context, evaluator),
            OpCode::Slice => array::evaluate_slice(args, context, evaluator),

            // Special operators - direct function calls
            OpCode::Missing => missing::evaluate_missing(args, context, evaluator),
            OpCode::MissingSome => missing::evaluate_missing_some(args, context, evaluator),
            OpCode::Try => try_op::evaluate_try(args, context, evaluator),
            OpCode::Throw => throw::evaluate_throw(args, context, evaluator),
            OpCode::Type => type_op::evaluate_type(args, context, evaluator),
            OpCode::Preserve => preserve::evaluate_preserve(args, context, evaluator),

            // DateTime operators - direct function calls
            OpCode::Datetime => datetime::evaluate_datetime(args, context, evaluator),
            OpCode::Timestamp => datetime::evaluate_timestamp(args, context, evaluator),
            OpCode::ParseDate => datetime::evaluate_parse_date(args, context, evaluator),
            OpCode::FormatDate => datetime::evaluate_format_date(args, context, evaluator),
            OpCode::DateDiff => datetime::evaluate_date_diff(args, context, evaluator),
            OpCode::Now => datetime::evaluate_now(args, context, evaluator),
        }
    }
}
