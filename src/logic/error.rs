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
    
    /// Error evaluating a logic expression.
    EvaluationError {
        /// The reason for the evaluation failure.
        reason: String,
    },
    
    /// Error with an operator.
    OperatorError {
        /// The name of the operator.
        operator: String,
        /// The reason for the operator failure.
        reason: String,
    },
    
    /// Error accessing a variable.
    VariableError {
        /// The variable path that caused the error.
        path: String,
        /// The reason for the variable access failure.
        reason: String,
    },
    
    /// Error with a type mismatch.
    TypeMismatch {
        /// The expected type.
        expected: String,
        /// The actual type found.
        found: String,
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
            LogicError::EvaluationError { reason } => {
                write!(f, "Evaluation error: {}", reason)
            }
            LogicError::OperatorError { operator, reason } => {
                write!(f, "Operator '{}' error: {}", operator, reason)
            }
            LogicError::VariableError { path, reason } => {
                write!(f, "Variable '{}' error: {}", path, reason)
            }
            LogicError::TypeMismatch { expected, found } => {
                write!(f, "Type mismatch: expected {}, found {}", expected, found)
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
    
    /// Creates an evaluation error with the given reason.
    pub fn evaluation_error(reason: impl Into<String>) -> Self {
        LogicError::EvaluationError {
            reason: reason.into(),
        }
    }
    
    /// Creates an operator error with the given operator name and reason.
    pub fn operator_error(operator: impl Into<String>, reason: impl Into<String>) -> Self {
        LogicError::OperatorError {
            operator: operator.into(),
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
    
    /// Creates a type mismatch error with the expected and found types.
    pub fn type_mismatch(expected: impl Into<String>, found: impl Into<String>) -> Self {
        LogicError::TypeMismatch {
            expected: expected.into(),
            found: found.into(),
        }
    }
    
    /// Creates a custom error with the given message.
    pub fn custom(message: impl Into<String>) -> Self {
        LogicError::Custom(message.into())
    }
    
    /// Creates an argument count error for an operator.
    pub fn argument_count_error(operator: impl Into<String>, expected: usize, got: usize) -> Self {
        LogicError::OperatorError {
            operator: operator.into(),
            reason: format!("Expected {} argument(s), got {}", expected, got),
        }
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
    fn test_evaluation_error() {
        let err = LogicError::EvaluationError {
            reason: "division by zero".to_string(),
        };
        assert_eq!(err.to_string(), "Evaluation error: division by zero");
    }

    #[test]
    fn test_operator_error() {
        let err = LogicError::OperatorError {
            operator: "+".to_string(),
            reason: "invalid operands".to_string(),
        };
        assert_eq!(err.to_string(), "Operator '+' error: invalid operands");
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
    fn test_type_mismatch() {
        let err = LogicError::TypeMismatch {
            expected: "number".to_string(),
            found: "string".to_string(),
        };
        assert_eq!(err.to_string(), "Type mismatch: expected number, found string");
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