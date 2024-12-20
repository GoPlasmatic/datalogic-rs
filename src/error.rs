//! Error types for JSON Logic operations
//!
//! This module provides comprehensive error handling for rule parsing and evaluation.

use thiserror::Error;

#[derive(Error, Debug)]
pub enum Error {
    #[error("Unknown operator: {0}")]
    UnknownOperator(String),

    #[error("Invalid rule format: {0}")]
    InvalidRule(String),
    
    #[error("Invalid arguments: {0}")]
    InvalidArguments(String),
    
    #[error("Type error: {0}")]
    Type(String),
}