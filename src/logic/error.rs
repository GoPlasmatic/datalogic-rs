//! Error types for logic operations.
//!
//! This module provides error types for operations involving logic expressions.

use std::error::Error;
use std::fmt;
use std::result;

/// A specialized Result type for logic operations.
pub type Result<T> = result::Result<T, LogicError>;

/// Errors that can occur during logic operations.
#[derive(Debug, Clone, PartialEq)]
pub enum LogicError {
    /// Error parsing a logic expression from JSON.
    ParseError {
        /// The reason for the parsing failure.
        reason: String,
    },
    
    /// Error accessing a variable.
    VariableError {
        /// The variable path that caused the error.
        path: String,
        /// The reason for the variable access failure.
        reason: String,
    },
    
    /// Error indicating that an operator is not found.
    OperatorNotFoundError {
        /// The operator that was not found.
        operator: String,
    },
    
    NaNError,

    InvalidArgumentsError,
    
    /// A custom error with a message.
    Custom(String),
}

impl fmt::Display for LogicError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            LogicError::ParseError { reason } => {
                write!(f, "Parse error: {}", reason)
            }
            LogicError::VariableError { path, reason } => {
                write!(f, "Variable '{}' error: {}", path, reason)
            }
            LogicError::NaNError => {
                write!(f, "NaN error")
            }
            LogicError::InvalidArgumentsError => {
                write!(f, "Invalid arguments error")
            }
            LogicError::Custom(msg) => {
                write!(f, "{}", msg)
            }
            LogicError::OperatorNotFoundError { operator } => {
                write!(f, "Operator '{}' not found", operator)
            }
        }
    }
}

impl Error for LogicError {}

/// Extension methods for Result<T, LogicError>.
pub trait LogicResultExt<T> {
    /// Adds context to an error with a custom message.
    fn with_context<F>(self, f: F) -> Result<T>
    where
        F: FnOnce() -> String;
}

impl<T> LogicResultExt<T> for Result<T> {
    fn with_context<F>(self, f: F) -> Result<T>
    where
        F: FnOnce() -> String,
    {
        self.map_err(|err| {
            let context = f();
            LogicError::Custom(format!("{}: {}", context, err))
        })
    }
}

impl LogicError {
    /// Creates a parse error with the given reason.
    pub fn parse_error(reason: impl Into<String>) -> Self {
        LogicError::ParseError {
            reason: reason.into(),
        }
    }
    
    /// Creates a variable error with the given path and reason.
    pub fn variable_error(path: impl Into<String>, reason: impl Into<String>) -> Self {
        LogicError::VariableError {
            path: path.into(),
            reason: reason.into(),
        }
    }
       
    /// Creates a custom error with the given message.
    pub fn custom(message: impl Into<String>) -> Self {
        LogicError::Custom(message.into())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_error() {
        let err = LogicError::ParseError {
            reason: "unexpected token".to_string(),
        };
        assert_eq!(err.to_string(), "Parse error: unexpected token");
    }

    #[test]
    fn test_variable_error() {
        let err = LogicError::VariableError {
            path: "user.age".to_string(),
            reason: "not found".to_string(),
        };
        assert_eq!(err.to_string(), "Variable 'user.age' error: not found");
    }

    #[test]
    fn test_with_context() {
        let result: Result<()> = Err(LogicError::ParseError {
            reason: "unexpected token".to_string(),
        });
        
        let result_with_context = result.with_context(|| "Failed to parse logic expression".to_string());
        
        assert!(result_with_context.is_err());
        if let Err(err) = result_with_context {
            if let LogicError::Custom(msg) = err {
                assert_eq!(msg, "Failed to parse logic expression: Parse error: unexpected token");
            } else {
                panic!("Expected Custom error variant");
            }
        }
    }
} 