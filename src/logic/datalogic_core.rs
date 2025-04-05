use super::error::Result;
use crate::arena::DataArena;
use crate::builder::factory::RuleFactory;
use crate::builder::RuleBuilder;
use crate::logic::{evaluate, Logic};
use crate::value::{DataValue, FromJson, ToJson};

/// The main engine implementation for DataLogic expressions.
///
/// This struct provides core methods for evaluating rule expressions,
/// as well as access to the builder API for creating rules.
pub struct DataLogicCore {
    /// The arena in which all allocations will be made.
    arena: DataArena,
}

impl DataLogicCore {
    /// Creates a new DataLogicCore instance with its own memory arena.
    pub fn new() -> Self {
        Self {
            arena: DataArena::new(),
        }
    }

    /// Gets a reference to the memory arena.
    pub fn arena(&self) -> &DataArena {
        &self.arena
    }

    /// Creates a rule builder that allocates in this instance's arena.
    pub fn builder(&self) -> RuleBuilder {
        RuleBuilder::new(&self.arena)
    }

    /// Creates a rule factory that allocates in this instance's arena.
    pub fn factory(&self) -> RuleFactory {
        RuleFactory::new(&self.arena)
    }

    /// Evaluates a rule created with the builder API.
    pub fn apply(&self, logic: &Logic, data: &serde_json::Value) -> Result<serde_json::Value> {
        // Convert input data to DataValue
        let data_value = DataValue::from_json(data, &self.arena);
        self.arena
            .set_current_context(&data_value, &DataValue::String("$"));
        self.arena.set_root_context(&data_value);

        // Evaluate the rule
        let result = evaluate(logic.root(), &self.arena)?;

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
