//! Error types for JSON Logic operations
//!
//! This module provides comprehensive error handling for rule parsing and evaluation.

use thiserror::Error;

#[derive(Error, Debug, Clone)]
pub enum Error {
    #[error("Unknown Expression: {0}")]
    UnknownExpression(String),

    #[error("Invalid Expression format: {0}")]
    InvalidExpression(String),
    
    #[error("Invalid Arguments: {0}")]
    InvalidArguments(String),
    
    #[error("Type error: {0}")]
    Type(String),

    #[error("{0}")]
    Custom(String),
}