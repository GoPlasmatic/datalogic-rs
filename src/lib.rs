mod compiled;
mod context;
mod datetime;
mod engine;
mod error;
mod opcode;
mod operators;
mod value_helpers;

pub use compiled::{CompiledLogic, CompiledNode};
pub use context::{ContextFrame, ContextStack};
pub use engine::DataLogic;
pub use error::Error;
pub use opcode::OpCode;

use serde_json::Value;

/// Result type for DataLogic operations
pub type Result<T> = std::result::Result<T, Error>;

/// Evaluator trait for recursive evaluation
pub trait Evaluator {
    fn evaluate(&self, logic: &Value, context: &mut ContextStack) -> Result<Value>;
}

/// Operator trait for all operators
pub trait Operator: Send + Sync {
    fn evaluate(
        &self,
        args: &[Value],
        context: &mut ContextStack,
        evaluator: &dyn Evaluator,
    ) -> Result<Value>;
}
