// Core types and functionality
pub use datalogic::{CustomOperator, DataLogic};
pub use error::LogicError;
pub use logic::{Logic, Result};
pub use value::{DataValue, FromDataValue, FromJson, IntoDataValue, ToJson};

// Re-export the simple operator types
pub use arena::{SimpleOperatorAdapter, SimpleOperatorFn};
// Re-export EvalContext as it's needed for CustomOperator implementations
pub use context::EvalContext;

// Internal modules with implementation details
pub mod context; // Made public as EvalContext is needed for CustomOperator trait
mod parser;

// Public modules
pub mod arena;
pub mod datalogic;
pub mod error;
pub mod logic;
pub mod value;
