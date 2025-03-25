// Core types and functionality
pub use builder::RuleBuilder;
pub use datalogic::DataLogic;
pub use error::LogicError;
pub use logic::{Logic, Result};
pub use value::{DataValue, FromDataValue, FromJson, IntoDataValue, ToJson};

// Internal modules with implementation details
mod arena;
mod parser;

// Public modules
pub mod builder;
pub mod datalogic;
pub mod error;
pub mod logic;
pub mod value;
