// Core types and functionality
pub use datalogic::DataLogic;
pub use value::{DataValue, FromJson, ToJson, IntoDataValue, FromDataValue};
pub use logic::{Logic, Result};
pub use error::LogicError;
pub use builder::RuleBuilder;

// Internal modules with implementation details
mod arena;
mod parser;

// Public modules
pub mod builder;
pub mod datalogic;
pub mod error;
pub mod logic;
pub mod value;
