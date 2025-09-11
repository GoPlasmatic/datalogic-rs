mod compiled;
mod context;
mod engine;
mod error;
mod operators;
mod value_helpers;

pub use compiled::{CompiledLogic, CompiledNode};
pub use context::{ContextFrame, ContextStack};
pub use engine::DataLogic;
pub use error::Error;

use serde_json::Value;
use std::borrow::Cow;

/// Result type for DataLogic operations
pub type Result<T> = std::result::Result<T, Error>;

/// Evaluator trait for recursive evaluation
pub trait Evaluator {
    fn evaluate<'a>(
        &self,
        logic: &Cow<'a, Value>,
        context: &mut ContextStack<'a>,
    ) -> Result<Cow<'a, Value>>;
}

/// Operator trait for all operators
pub trait Operator: Send + Sync {
    fn evaluate<'a>(
        &self,
        args: &[Cow<'a, Value>],
        context: &mut ContextStack<'a>,
        evaluator: &dyn Evaluator,
    ) -> Result<Cow<'a, Value>>;
}
