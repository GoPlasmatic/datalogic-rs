// Core types and functionality
pub use builder::RuleBuilder;
pub use datalogic::{CustomOperator, DataLogic};
pub use error::LogicError;
pub use logic::{Logic, Result};
pub use value::{DataValue, FromDataValue, FromJson, IntoDataValue, ToJson};

// Re-export the simple operator types
pub use arena::{SimpleOperatorAdapter, SimpleOperatorFn};

// Internal modules with implementation details
mod parser;

// Public modules
pub mod arena;
pub mod builder;
pub mod datalogic;
pub mod error;
pub mod logic;
pub mod value;
