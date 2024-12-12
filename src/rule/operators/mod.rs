use serde_json::Value;
use crate::Error;
use super::Rule;

pub mod arithmetic;
pub mod array;
pub mod comparison;
pub mod logic;
pub mod missing;
pub mod preserve;
pub mod string;
pub mod var;
pub mod control;

pub use arithmetic::*;
pub use array::*;
pub use comparison::*;
pub use logic::*;
pub use missing::*;
pub use preserve::*;
pub use string::*;
pub use var::*;
pub use control::*;

/// Trait defining the interface for all JSONLogic operators
pub trait Operator {
    /// Apply the operator with given arguments and data
    fn apply(&self, args: &[Rule], data: &Value) -> Result<Value, Error>;
}