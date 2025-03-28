//! Unified error handling for the DataLogic library

// Re-export error types from logic module
pub use crate::logic::error::{LogicError, Result};

// Note: For backward compatibility, we're keeping the re-exports above.
// In a future version, consider replacing with a more comprehensive error type:
/*
pub enum Error {
    Parse(String),
    Evaluation(String),
    InvalidOperation(String),
    UndefinedVariable(String),
    TypeMismatch { expected: String, found: String },
    InvalidArgument(String),
    Custom(String),
}

pub type Result<T> = std::result::Result<T, Error>;
*/
