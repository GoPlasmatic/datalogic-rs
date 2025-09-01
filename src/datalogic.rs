//! Main interface for the DataLogic library
//!
//! This module provides the DataLogic struct which is the primary entry point
//! for parsing and evaluating logic expressions.

use crate::LogicError;
use crate::arena::{ArenaRef, DataArena};
use crate::arena::{CustomOperatorRegistry, SimpleOperatorAdapter, SimpleOperatorFn};
use crate::context::EvalContext;
use crate::logic::{Logic, Result, Token, evaluate, optimize};
use crate::parser::{
    parse_jsonlogic, parse_jsonlogic_json, parse_jsonlogic_json_with_preserve,
    parse_jsonlogic_with_preserve,
};
use crate::value::{DataValue, FromJson, ToJson};
use serde_json::Value as JsonValue;

/// Trait for custom JSONLogic operators
pub use crate::arena::CustomOperator;

/// Main interface for the DataLogic library
///
/// DataLogic can be created with either an internal arena (default) or
/// an external arena for sharing compiled logic across instances.
///
/// # Examples
///
/// ## Basic usage with internal arena
/// ```
/// use datalogic_rs::DataLogic;
///
/// let dl = DataLogic::new();
/// let result = dl.evaluate_str(
///     r#"{ ">": [{"var": "temp"}, 100] }"#,
///     r#"{"temp": 110, "name": "user"}"#,
/// ).unwrap();
/// assert_eq!(result.to_string(), "true");
/// ```
///
/// ## Sharing compiled logic with external arena
/// ```
/// use datalogic_rs::{DataLogic, DataArena};
///
/// // Create a shared logic arena
/// let logic_arena = DataArena::new();
///
/// // Create multiple instances sharing the logic arena
/// let mut dl1 = DataLogic::with_external_arena(&logic_arena);
/// let mut dl2 = DataLogic::with_external_arena(&logic_arena);
///
/// // Parse logic once
/// let rule = dl1.parse_logic(r#"{"==": [{"var": "x"}, 10]}"#).unwrap();
///
/// // Evaluate with different instances
/// let data1 = dl1.parse_data(r#"{"x": 10}"#).unwrap();
/// let result1 = dl1.evaluate_parsed(rule.root(), data1).unwrap();
///
/// let data2 = dl2.parse_data(r#"{"x": 20}"#).unwrap();
/// let result2 = dl2.evaluate_parsed(rule.root(), data2).unwrap();
/// ```
pub struct DataLogic<'logic> {
    /// Arena for compiled logic (can be external)
    logic_arena: ArenaRef<'logic>,
    /// Arena for evaluation data (always internal)
    eval_arena: DataArena,
    /// Whether to preserve structure
    preserve_structure: bool,
    /// Custom operators registry
    custom_operators: CustomOperatorRegistry,
}

/// Type alias for DataLogic with owned arena (backward compatibility)
pub type DataLogicOwned = DataLogic<'static>;

// Backward compatible constructors for owned arena
impl DataLogic<'static> {
    /// Create a new DataLogic instance with internal arenas
    pub fn new() -> Self {
        Self {
            logic_arena: ArenaRef::Owned(DataArena::new()),
            eval_arena: DataArena::new(),
            preserve_structure: false,
            custom_operators: CustomOperatorRegistry::new(),
        }
    }

    /// Create a new DataLogic instance with a specific chunk size for the arenas
    pub fn with_chunk_size(chunk_size: usize) -> Self {
        Self {
            logic_arena: ArenaRef::Owned(DataArena::with_chunk_size(chunk_size)),
            eval_arena: DataArena::with_chunk_size(chunk_size),
            preserve_structure: false,
            custom_operators: CustomOperatorRegistry::new(),
        }
    }

    /// Create a new DataLogic instance with structure preservation enabled
    ///
    /// When enabled, multi-key objects will preserve their structure and evaluate
    /// inner values instead of throwing OperatorNotFoundError.
    ///
    /// # Examples
    ///
    /// ```
    /// use datalogic_rs::DataLogic;
    ///
    /// let dl = DataLogic::with_preserve_structure();
    /// let result = dl.evaluate_str(
    ///     r#"{"isEqual": {"==": [1, 1]}}"#,
    ///     r#"{}"#,
    /// ).unwrap();
    /// // Returns: {"isEqual": true}
    /// ```
    pub fn with_preserve_structure() -> Self {
        Self {
            logic_arena: ArenaRef::Owned(DataArena::new()),
            eval_arena: DataArena::new(),
            preserve_structure: true,
            custom_operators: CustomOperatorRegistry::new(),
        }
    }

    /// Create a new DataLogic instance with both custom chunk size and structure preservation
    pub fn with_chunk_size_and_preserve_structure(chunk_size: usize) -> Self {
        Self {
            logic_arena: ArenaRef::Owned(DataArena::with_chunk_size(chunk_size)),
            eval_arena: DataArena::with_chunk_size(chunk_size),
            preserve_structure: true,
            custom_operators: CustomOperatorRegistry::new(),
        }
    }
}

// Generic implementation for any lifetime
impl<'logic> DataLogic<'logic> {
    /// Create a new DataLogic instance with an external logic arena
    ///
    /// This allows sharing compiled logic across multiple DataLogic instances.
    ///
    /// # Examples
    ///
    /// ```
    /// use datalogic_rs::{DataLogic, DataArena};
    ///
    /// let logic_arena = DataArena::new();
    /// let mut dl = DataLogic::with_external_arena(&logic_arena);
    /// ```
    pub fn with_external_arena(logic_arena: &'logic DataArena) -> Self {
        Self {
            logic_arena: ArenaRef::Borrowed(logic_arena),
            eval_arena: DataArena::new(),
            preserve_structure: false,
            custom_operators: CustomOperatorRegistry::new(),
        }
    }

    /// Create with external arena and custom evaluation chunk size
    pub fn with_external_arena_and_chunk_size(
        logic_arena: &'logic DataArena,
        eval_chunk_size: usize,
    ) -> Self {
        Self {
            logic_arena: ArenaRef::Borrowed(logic_arena),
            eval_arena: DataArena::with_chunk_size(eval_chunk_size),
            preserve_structure: false,
            custom_operators: CustomOperatorRegistry::new(),
        }
    }

    /// Create with external arena and structure preservation
    pub fn with_external_arena_and_preserve_structure(logic_arena: &'logic DataArena) -> Self {
        Self {
            logic_arena: ArenaRef::Borrowed(logic_arena),
            eval_arena: DataArena::new(),
            preserve_structure: true,
            custom_operators: CustomOperatorRegistry::new(),
        }
    }

    /// Get a reference to the logic arena
    pub fn logic_arena(&self) -> &DataArena {
        self.logic_arena.as_arena()
    }

    /// Get a reference to the logic arena with the proper lifetime for Logic creation
    /// This is only valid when Self: 'logic
    fn logic_arena_for_logic(&self) -> &'logic DataArena
    where
        Self: 'logic,
    {
        // SAFETY: When Self: 'logic, we know that either:
        // - For Borrowed: the arena has lifetime 'logic
        // - For Owned: self lives for 'logic, so the arena does too
        unsafe { std::mem::transmute::<&DataArena, &'logic DataArena>(self.logic_arena.as_arena()) }
    }

    /// Get a reference to the evaluation arena
    pub fn eval_arena(&self) -> &DataArena {
        &self.eval_arena
    }

    /// Get a reference to the internal arena (deprecated, use logic_arena or eval_arena)
    #[deprecated(note = "Use logic_arena() or eval_arena() instead")]
    pub fn arena(&self) -> &DataArena {
        &self.eval_arena
    }

    /// Check if structure preservation is enabled
    pub fn preserve_structure(&self) -> bool {
        self.preserve_structure
    }

    /// Reset evaluation arena (keeps logic intact)
    pub fn reset_eval_arena(&mut self) {
        self.eval_arena.reset();
    }

    /// Reset both arenas (logic arena only if owned)
    pub fn reset_all(&mut self) {
        self.logic_arena.reset(); // No-op if borrowed
        self.eval_arena.reset();
    }

    /// Reset the internal arena to free memory (deprecated, use reset_eval_arena or reset_all)
    #[deprecated(note = "Use reset_eval_arena() or reset_all() instead")]
    pub fn reset_arena(&mut self) {
        self.reset_all();
    }

    /// Register a custom operator implementation
    ///
    /// This allows users to extend JSONLogic with custom operations.
    pub fn register_custom_operator(&mut self, name: &str, operator: Box<dyn CustomOperator>) {
        self.custom_operators.register(name, operator);
    }

    /// Check if a custom operator is registered
    pub fn has_custom_operator(&self, name: &str) -> bool {
        self.custom_operators.get(name).is_some()
    }

    /// Parse a logic expression from a string (compile to tokens in logic arena)
    pub fn parse_logic(&self, source: &str) -> Result<Logic<'logic>>
    where
        Self: 'logic,
    {
        let arena = self.logic_arena_for_logic();
        let token = if self.preserve_structure {
            parse_jsonlogic_with_preserve(source, arena, true, &self.custom_operators)?
        } else {
            parse_jsonlogic(source, arena, &self.custom_operators)?
        };

        // Apply static optimization
        let optimized_token = optimize(token, arena)?;

        Ok(Logic::new(optimized_token, arena))
    }

    /// Parse a JSON logic expression into a Token
    pub fn parse_logic_json(&self, source: &JsonValue) -> Result<Logic<'logic>>
    where
        Self: 'logic,
    {
        let arena = self.logic_arena_for_logic();
        let token = if self.preserve_structure {
            parse_jsonlogic_json_with_preserve(source, arena, true, &self.custom_operators)?
        } else {
            parse_jsonlogic_json(source, arena, &self.custom_operators)?
        };
        Ok(Logic::new(token, arena))
    }

    /// Parse a JSON data string into a DataValue (uses eval arena)
    pub fn parse_data(&self, source: &str) -> Result<&DataValue<'_>> {
        let json = serde_json::from_str(source).map_err(|e| LogicError::ParseError {
            reason: e.to_string(),
        })?;
        let value = DataValue::from_json(&json, &self.eval_arena);
        Ok(self.eval_arena.alloc(value))
    }

    /// Parse a JSON data into a DataValue (uses eval arena)
    pub fn parse_data_json(&self, source: &JsonValue) -> Result<&DataValue<'_>> {
        let value = DataValue::from_json(source, &self.eval_arena);
        Ok(self.eval_arena.alloc(value))
    }

    /// Evaluate pre-parsed logic with data
    ///
    /// This method allows evaluating logic that was parsed separately,
    /// potentially by a different DataLogic instance sharing the same logic arena.
    ///
    /// # Arguments
    ///
    /// * `logic` - Pre-parsed logic token (can be from any arena)
    /// * `data` - The data to use as context during evaluation
    ///
    /// # Examples
    ///
    /// ```
    /// use datalogic_rs::{DataLogic, DataArena};
    ///
    /// let logic_arena = DataArena::new();
    /// let mut dl = DataLogic::with_external_arena(&logic_arena);
    ///
    /// let rule = dl.parse_logic(r#"{"var": "x"}"#).unwrap();
    /// let data = dl.parse_data(r#"{"x": 42}"#).unwrap();
    /// let result = dl.evaluate_parsed(rule.root(), data).unwrap();
    /// assert_eq!(result.as_i64(), Some(42));
    /// ```
    pub fn evaluate_parsed<'a>(
        &'a self,
        logic: &'a Token<'a>,
        data: &'a DataValue<'a>,
    ) -> Result<&'a DataValue<'a>> {
        let context = EvalContext::new(data, &self.custom_operators);
        evaluate(logic, &context, &self.eval_arena)
    }

    /// Evaluate a rule with the provided data
    ///
    /// This method evaluates a logic rule against the given data context.
    ///
    /// # Arguments
    ///
    /// * `rule` - The compiled logic rule to evaluate
    /// * `data` - The data to use as context during evaluation
    pub fn evaluate<'a>(
        &'a self,
        rule: &'a Logic,
        data: &'a DataValue,
    ) -> Result<&'a DataValue<'a>> {
        self.evaluate_parsed(rule.root(), data)
    }

    /// Evaluate using JSON values directly
    pub fn evaluate_json(&self, logic: &JsonValue, data: &JsonValue) -> Result<JsonValue> {
        let rule = self.parse_logic_json(logic)?;
        let data_value = self.parse_data_json(data)?;
        let result = self.evaluate(&rule, data_value)?;
        Ok(result.to_json())
    }

    /// Parse and evaluate in one step, returning a JSON value (backward compatible)
    pub fn evaluate_str(&self, logic_source: &str, data_source: &str) -> Result<JsonValue> {
        let rule = self.parse_logic(logic_source)?;
        let data_value = self.parse_data(data_source)?;
        let result = self.evaluate(&rule, data_value)?;
        Ok(result.to_json())
    }

    /// Register a simple custom operator implementation
    pub fn register_simple_operator(&mut self, name: &str, function: SimpleOperatorFn) {
        let adapter = SimpleOperatorAdapter::new(name, function);
        self.register_custom_operator(name, Box::new(adapter));
    }
}

impl Default for DataLogic<'static> {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::value::NumberValue;
    use serde_json::json;

    #[derive(Debug)]
    struct MultiplyAll;

    impl CustomOperator for MultiplyAll {
        fn evaluate<'a>(
            &self,
            args: &'a [DataValue<'a>],
            _context: &crate::context::EvalContext<'a>,
            arena: &'a DataArena,
        ) -> Result<&'a DataValue<'a>> {
            if args.is_empty() {
                return Ok(arena.alloc(DataValue::Number(NumberValue::from_i64(1))));
            }

            let mut product = 1.0;
            for arg in args {
                if let Some(n) = arg.as_f64() {
                    product *= n;
                }
            }

            Ok(arena.alloc(DataValue::Number(NumberValue::from_f64(product))))
        }
    }

    #[test]
    fn test_custom_operator() {
        let mut dl = DataLogic::new();
        dl.register_custom_operator("multiply_all", Box::new(MultiplyAll));

        let result = dl
            .evaluate_json(&json!({"multiply_all": [2, 3, 4]}), &json!({}))
            .unwrap();
        assert_eq!(result.as_f64().unwrap(), 24.0);
    }

    #[test]
    fn test_external_arena() {
        // Create a shared logic arena
        let logic_arena = DataArena::new();

        // Parse logic once using a temporary DataLogic
        let rule = {
            let dl = DataLogic::with_external_arena(&logic_arena);
            dl.parse_logic(r#"{"==": [{"var": "x"}, 10]}"#).unwrap()
        };

        // Create two DataLogic instances sharing the logic arena
        let mut dl1 = DataLogic::with_external_arena(&logic_arena);
        let mut dl2 = DataLogic::with_external_arena(&logic_arena);

        // Evaluate with dl1
        let data1 = dl1.parse_data(r#"{"x": 10}"#).unwrap();
        let result1 = dl1.evaluate_parsed(rule.root(), data1).unwrap();
        assert_eq!(result1.as_bool(), Some(true));

        // Evaluate same rule with dl2
        let data2 = dl2.parse_data(r#"{"x": 20}"#).unwrap();
        let result2 = dl2.evaluate_parsed(rule.root(), data2).unwrap();
        assert_eq!(result2.as_bool(), Some(false));

        // Reset eval arenas without affecting shared logic
        dl1.reset_eval_arena();
        dl2.reset_eval_arena();

        // Rule should still be valid
        let data3 = dl1.parse_data(r#"{"x": 10}"#).unwrap();
        let result3 = dl1.evaluate_parsed(rule.root(), data3).unwrap();
        assert_eq!(result3.as_bool(), Some(true));
    }

    #[test]
    fn test_arena_isolation() {
        let logic_arena = DataArena::new();

        // Compile rule using a temporary DataLogic
        let rule = {
            let dl = DataLogic::with_external_arena(&logic_arena);
            dl.parse_logic(r#"{"var": "x"}"#).unwrap()
        };

        // Evaluate in a different instance
        let dl2 = DataLogic::with_external_arena(&logic_arena);
        let data = dl2.parse_data(r#"{"x": 42}"#).unwrap();
        let result = dl2.evaluate_parsed(rule.root(), data).unwrap();

        assert_eq!(result.as_i64(), Some(42));
    }

    #[test]
    fn test_backward_compatibility() {
        // Old API should still work
        let dl = DataLogic::new();
        let result = dl.evaluate_str(r#"{"==": [1, 1]}"#, r#"{}"#).unwrap();
        assert_eq!(result, json!(true));

        // Test with structure preservation
        let dl2 = DataLogic::with_preserve_structure();
        let result2 = dl2
            .evaluate_str(r#"{"result": {"==": [1, 1]}}"#, r#"{}"#)
            .unwrap();
        assert_eq!(result2, json!({"result": true}));
    }
}
