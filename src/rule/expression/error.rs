//! Error types for JSON Logic operations
//!
//! This module provides comprehensive error handling for rule parsing and evaluation.

use thiserror::Error;

#[derive(Error, Debug)]
pub enum Error {
    #[error("Unknown Expression: {0}")]
    UnknownExpression(String),

    #[error("Invalid Expression format: {0}")]
    InvalidExpression(String),
    
    #[error("Invalid arguments: {0}")]
    InvalidArguments(String),
    
    #[error("Type error: {0}")]
    Type(String),
}