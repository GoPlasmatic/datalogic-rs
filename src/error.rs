use std::fmt;

/// Error type for DataLogic operations
#[derive(Debug, Clone)]
pub enum Error {
    /// Invalid operator name
    InvalidOperator(String),

    /// Invalid arguments for an operator
    InvalidArguments(String),

    /// Variable not found in context
    VariableNotFound(String),

    /// Invalid context level access
    InvalidContextLevel(isize),

    /// Type conversion/coercion error
    TypeError(String),

    /// Arithmetic error (division by zero, overflow, etc.)
    ArithmeticError(String),

    /// Custom error for extensions
    Custom(String),

    /// JSON parsing/serialization error
    ParseError(String),

    /// Thrown error from throw operator
    Thrown(serde_json::Value),

    /// Invalid format string or pattern
    FormatError(String),

    /// Index out of bounds for array operations
    IndexOutOfBounds { index: isize, length: usize },

    /// Invalid operator configuration
    ConfigurationError(String),
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Error::InvalidOperator(op) => write!(f, "Invalid operator: {}", op),
            Error::InvalidArguments(msg) => write!(f, "Invalid arguments: {}", msg),
            Error::VariableNotFound(var) => write!(f, "Variable not found: {}", var),
            Error::InvalidContextLevel(level) => write!(f, "Invalid context level: {}", level),
            Error::TypeError(msg) => write!(f, "Type error: {}", msg),
            Error::ArithmeticError(msg) => write!(f, "Arithmetic error: {}", msg),
            Error::Custom(msg) => write!(f, "{}", msg),
            Error::ParseError(msg) => write!(f, "Parse error: {}", msg),
            Error::Thrown(val) => write!(f, "Thrown: {}", val),
            Error::FormatError(msg) => write!(f, "Format error: {}", msg),
            Error::IndexOutOfBounds { index, length } => {
                write!(
                    f,
                    "Index {} out of bounds for array of length {}",
                    index, length
                )
            }
            Error::ConfigurationError(msg) => write!(f, "Configuration error: {}", msg),
        }
    }
}

impl std::error::Error for Error {}

impl From<serde_json::Error> for Error {
    fn from(err: serde_json::Error) -> Self {
        Error::ParseError(err.to_string())
    }
}
