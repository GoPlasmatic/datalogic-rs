use super::error::Result;
use crate::arena::{CustomOperatorRegistry, DataArena};
use crate::context::EvalContext;
use crate::logic::{Logic, evaluate};
use crate::value::{DataValue, FromJson, ToJson};

/// The main engine implementation for DataLogic expressions.
///
/// This struct provides core methods for evaluating rule expressions.
pub struct DataLogicCore {
    /// The arena in which all allocations will be made.
    arena: DataArena,
    /// Custom operator registry for this core instance
    custom_operators: CustomOperatorRegistry,
}

impl DataLogicCore {
    /// Creates a new DataLogicCore instance with its own memory arena.
    pub fn new() -> Self {
        Self {
            arena: DataArena::new(),
            custom_operators: CustomOperatorRegistry::new(),
        }
    }

    /// Gets a reference to the memory arena.
    pub fn arena(&self) -> &DataArena {
        &self.arena
    }

    /// Evaluates a logic rule.
    pub fn apply(&self, logic: &Logic, data: &serde_json::Value) -> Result<serde_json::Value> {
        // Convert input data to DataValue
        let data_value = DataValue::from_json(data, &self.arena);
        let data_ref = self.arena.alloc(data_value);

        // Create evaluation context with the data as root
        let context = EvalContext::new(data_ref, &self.custom_operators);

        // Evaluate the rule
        let result = evaluate(logic.root(), &context, &self.arena)?;

        // Convert the result back to a JSON value
        Ok(result.to_json())
    }
}

// Default implementation to make it easier to create instances
impl Default for DataLogicCore {
    fn default() -> Self {
        Self::new()
    }
}
