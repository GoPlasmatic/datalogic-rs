use crate::arena::DataArena;
use crate::value::{DataValue, FromJson, ToJson};
use crate::logic::{Logic, IntoLogic, evaluate};
use crate::builder::RuleBuilder;
use crate::builder::factory::RuleFactory;
use super::error::{LogicError, Result};

/// The main entry point for the JSONLogic library.
///
/// This struct provides methods for evaluating JSONLogic expressions,
/// as well as access to the builder API for creating rules.
pub struct JsonLogic {
    /// The arena in which all allocations will be made.
    arena: DataArena,
}

impl JsonLogic {
    /// Creates a new JSONLogic instance with its own memory arena.
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
    
    /// Parses a JSONLogic rule from a JSON value.
    pub fn parse<T: IntoLogic>(&self, rule: &T) -> Result<Logic> {
        rule.to_logic(&self.arena)
    }
    
    /// Applies a JSONLogic rule to input data.
    pub fn apply<T: IntoLogic>(&self, rule: &T, data: &serde_json::Value) -> Result<serde_json::Value> {
        // Parse the rule
        let logic = rule.to_logic(&self.arena)?;
        
        // Convert input data to DataValue
        let data_value = DataValue::from_json(data, &self.arena);
        
        // Evaluate the rule
        let result = evaluate(logic.root(), &data_value, &self.arena)?;
        
        // Convert the result back to a JSON value
        Ok(result.to_json())
    }
    
    /// Evaluates a rule created with the builder API.
    pub fn apply_logic(&self, logic: &Logic, data: &serde_json::Value) -> Result<serde_json::Value> {
        // Convert input data to DataValue
        let data_value = DataValue::from_json(data, &self.arena);
        
        // Evaluate the rule
        let result = evaluate(logic.root(), &data_value, &self.arena)?;
        
        // Convert the result back to a JSON value
        Ok(result.to_json())
    }
    
    /// Creates a new JSONLogic instance from a JSON value.
    pub fn from_json(json: &serde_json::Value) -> Result<Self> {
        let instance = Self::new();
        let _logic = json.to_logic(&instance.arena)?;
        Ok(instance)
    }
    
    /// Creates a new JSONLogic instance from a JSON string.
    pub fn from_str(json_str: &str) -> Result<Self> {
        let json: serde_json::Value = serde_json::from_str(json_str)
            .map_err(|e| LogicError::ParseError {
                reason: format!("Invalid JSON: {}", e),
            })?;
        Self::from_json(&json)
    }
}

// Default implementation to make it easier to create instances
impl Default for JsonLogic {
    fn default() -> Self {
        Self::new()
    }
} 