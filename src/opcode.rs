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

    // Math operators
    Abs = 49,
    Ceil = 50,
    Floor = 51,

    // Utility operators
    Preserve = 52,
    Sort = 54,
    Slice = 55,
}

impl OpCode {
    /// Total number of built-in operators
    pub const COUNT: usize = 56;

    /// Convert a string to an OpCode
    pub fn from_str(s: &str) -> Option<OpCode> {
        match s {
            "var" => Some(OpCode::Var),
            "val" => Some(OpCode::Val),
            "==" => Some(OpCode::Equals),
            "===" => Some(OpCode::StrictEquals),
            "!=" => Some(OpCode::NotEquals),
            "!==" => Some(OpCode::StrictNotEquals),
            ">" => Some(OpCode::GreaterThan),
            ">=" => Some(OpCode::GreaterThanEqual),
            "<" => Some(OpCode::LessThan),
            "<=" => Some(OpCode::LessThanEqual),
            "!" => Some(OpCode::Not),
            "!!" => Some(OpCode::DoubleNot),
            "and" => Some(OpCode::And),
            "or" => Some(OpCode::Or),
            "if" => Some(OpCode::If),
            "?:" => Some(OpCode::Ternary),
            "+" => Some(OpCode::Add),
            "-" => Some(OpCode::Subtract),
            "*" => Some(OpCode::Multiply),
            "/" => Some(OpCode::Divide),
            "%" => Some(OpCode::Modulo),
            "max" => Some(OpCode::Max),
            "min" => Some(OpCode::Min),
            "cat" => Some(OpCode::Cat),
            "substr" => Some(OpCode::Substr),
            "in" => Some(OpCode::In),
            "merge" => Some(OpCode::Merge),
            "filter" => Some(OpCode::Filter),
            "map" => Some(OpCode::Map),
            "reduce" => Some(OpCode::Reduce),
            "all" => Some(OpCode::All),
            "some" => Some(OpCode::Some),
            "none" => Some(OpCode::None),
            "missing" => Some(OpCode::Missing),
            "missing_some" => Some(OpCode::MissingSome),
            "try" => Some(OpCode::Try),
            "throw" => Some(OpCode::Throw),
            "type" => Some(OpCode::Type),
            "starts_with" => Some(OpCode::StartsWith),
            "ends_with" => Some(OpCode::EndsWith),
            "upper" => Some(OpCode::Upper),
            "lower" => Some(OpCode::Lower),
            "trim" => Some(OpCode::Trim),
            "split" => Some(OpCode::Split),
            "datetime" => Some(OpCode::Datetime),
            "timestamp" => Some(OpCode::Timestamp),
            "parse_date" => Some(OpCode::ParseDate),
            "format_date" => Some(OpCode::FormatDate),
            "date_diff" => Some(OpCode::DateDiff),
            "abs" => Some(OpCode::Abs),
            "ceil" => Some(OpCode::Ceil),
            "floor" => Some(OpCode::Floor),
            "preserve" => Some(OpCode::Preserve),
            "length" => Some(OpCode::Length),
            "sort" => Some(OpCode::Sort),
            "slice" => Some(OpCode::Slice),
            _ => None,
        }
    }

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
            OpCode::Abs => "abs",
            OpCode::Ceil => "ceil",
            OpCode::Floor => "floor",
            OpCode::Preserve => "preserve",
            OpCode::Length => "length",
            OpCode::Sort => "sort",
            OpCode::Slice => "slice",
        }
    }
}
