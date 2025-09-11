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
}

impl OpCode {
    /// Total number of built-in operators
    pub const COUNT: usize = 35;

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
        }
    }
}
