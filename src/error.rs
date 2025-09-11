use std::fmt;

/// Error type for DataLogic operations
#[derive(Debug, Clone)]
pub enum Error {
    /// Invalid operator name
    InvalidOperator(String),

    /// Invalid arguments for an operator
    InvalidArguments(String),

    /// Variable not found
    VariableNotFound(String),

    /// Invalid context level
    InvalidContextLevel(isize),

    /// Type conversion error
    TypeError(String),

    /// Division by zero
    DivisionByZero,

    /// Custom error
    Custom(String),

    /// JSON parsing error
    ParseError(String),

    /// Thrown error from throw operator
    Thrown(serde_json::Value),
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Error::InvalidOperator(op) => write!(f, "Invalid operator: {}", op),
            Error::InvalidArguments(msg) => write!(f, "Invalid arguments: {}", msg),
            Error::VariableNotFound(var) => write!(f, "Variable not found: {}", var),
            Error::InvalidContextLevel(level) => write!(f, "Invalid context level: {}", level),
            Error::TypeError(msg) => write!(f, "Type error: {}", msg),
            Error::DivisionByZero => write!(f, "Division by zero"),
            Error::Custom(msg) => write!(f, "{}", msg),
            Error::ParseError(msg) => write!(f, "Parse error: {}", msg),
            Error::Thrown(val) => write!(f, "Thrown: {}", val),
        }
    }
}

impl std::error::Error for Error {}

impl From<serde_json::Error> for Error {
    fn from(err: serde_json::Error) -> Self {
        Error::ParseError(err.to_string())
    }
}
