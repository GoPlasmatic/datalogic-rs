use serde_json::Value;
use smallvec::SmallVec;
use std::collections::HashMap;
use std::sync::Arc;

use crate::config::EvaluationConfig;
use crate::{CompiledLogic, CompiledNode, ContextStack, Error, Evaluator, Operator, Result};

/// The main DataLogic engine for compiling and evaluating JSONLogic expressions.
///
/// The engine provides a two-phase approach to logic evaluation:
/// 1. **Compilation**: Parse JSON logic into optimized `CompiledLogic`
/// 2. **Evaluation**: Execute compiled logic against data
///
/// # Features
///
/// - **Thread-safe**: Compiled logic can be shared across threads with `Arc`
/// - **Extensible**: Add custom operators via `add_operator`
/// - **Structure preservation**: Optionally preserve object structure for templating
/// - **OpCode dispatch**: Built-in operators use fast enum-based dispatch
///
/// # Example
///
/// ```rust
/// use datalogic_rs::DataLogic;
/// use serde_json::json;
///
/// let engine = DataLogic::new();
/// let logic = json!({">": [{"var": "age"}, 18]});
/// let compiled = engine.compile(&logic).unwrap();
///
/// let data = json!({"age": 21});
/// let result = engine.evaluate_owned(&compiled, data).unwrap();
/// assert_eq!(result, json!(true));
/// ```
pub struct DataLogic {
    // No more builtin_operators array - OpCode handles dispatch directly!
    /// HashMap for custom operators only
    custom_operators: HashMap<String, Box<dyn Operator>>,
    /// Flag to preserve structure of objects with unknown operators
    preserve_structure: bool,
    /// Configuration for evaluation behavior
    config: EvaluationConfig,
}

impl Default for DataLogic {
    fn default() -> Self {
        Self::new()
    }
}

impl DataLogic {
    /// Creates a new DataLogic engine with all built-in operators.
    ///
    /// The engine includes 50+ built-in operators optimized with OpCode dispatch.
    /// Structure preservation is disabled by default.
    ///
    /// # Example
    ///
    /// ```rust
    /// use datalogic_rs::DataLogic;
    ///
    /// let engine = DataLogic::new();
    /// ```
    pub fn new() -> Self {
        Self {
            custom_operators: HashMap::new(),
            preserve_structure: false,
            config: EvaluationConfig::default(),
        }
    }

    /// Creates a new DataLogic engine with structure preservation enabled.
    ///
    /// When enabled, objects with unknown operators are preserved as structured
    /// templates, allowing for dynamic object generation. Custom operators
    /// registered via `add_operator` are recognized and evaluated properly,
    /// even within structured objects.
    ///
    /// # Example
    ///
    /// ```rust
    /// use datalogic_rs::DataLogic;
    /// use serde_json::json;
    ///
    /// let engine = DataLogic::with_preserve_structure();
    /// let logic = json!({
    ///     "name": {"var": "user.name"},
    ///     "score": {"+": [{"var": "base"}, {"var": "bonus"}]}
    /// });
    /// // Returns: {"name": "Alice", "score": 95}
    /// ```
    ///
    /// # Custom Operators with Preserve Structure
    ///
    /// Custom operators work seamlessly in preserve_structure mode:
    ///
    /// ```rust
    /// use datalogic_rs::{DataLogic, Operator, ContextStack, Evaluator, Result, Error};
    /// use serde_json::{json, Value};
    /// use std::sync::Arc;
    ///
    /// struct UpperOperator;
    /// impl Operator for UpperOperator {
    ///     fn evaluate(&self, args: &[Value], context: &mut ContextStack,
    ///                 evaluator: &dyn Evaluator) -> Result<Value> {
    ///         let val = evaluator.evaluate(&args[0], context)?;
    ///         Ok(json!(val.as_str().unwrap_or("").to_uppercase()))
    ///     }
    /// }
    ///
    /// let mut engine = DataLogic::with_preserve_structure();
    /// engine.add_operator("upper".to_string(), Box::new(UpperOperator));
    ///
    /// let logic = json!({
    ///     "message": {"upper": {"var": "text"}},
    ///     "count": {"var": "num"}
    /// });
    /// let compiled = engine.compile(&logic).unwrap();
    /// let result = engine.evaluate(&compiled, Arc::new(json!({"text": "hello", "num": 5}))).unwrap();
    /// // Returns: {"message": "HELLO", "count": 5}
    /// ```
    pub fn with_preserve_structure() -> Self {
        Self {
            custom_operators: HashMap::new(),
            preserve_structure: true,
            config: EvaluationConfig::default(),
        }
    }

    /// Creates a new DataLogic engine with a custom configuration.
    ///
    /// # Arguments
    ///
    /// * `config` - The evaluation configuration
    ///
    /// # Example
    ///
    /// ```rust
    /// use datalogic_rs::{DataLogic, EvaluationConfig, NanHandling};
    ///
    /// let config = EvaluationConfig::default()
    ///     .with_nan_handling(NanHandling::IgnoreValue);
    /// let engine = DataLogic::with_config(config);
    /// ```
    pub fn with_config(config: EvaluationConfig) -> Self {
        Self {
            custom_operators: HashMap::new(),
            preserve_structure: false,
            config,
        }
    }

    /// Creates a new DataLogic engine with both configuration and structure preservation.
    ///
    /// # Arguments
    ///
    /// * `config` - The evaluation configuration
    /// * `preserve_structure` - Whether to preserve object structure
    ///
    /// # Example
    ///
    /// ```rust
    /// use datalogic_rs::{DataLogic, EvaluationConfig, NanHandling};
    ///
    /// let config = EvaluationConfig::default()
    ///     .with_nan_handling(NanHandling::IgnoreValue);
    /// let engine = DataLogic::with_config_and_structure(config, true);
    /// ```
    pub fn with_config_and_structure(config: EvaluationConfig, preserve_structure: bool) -> Self {
        Self {
            custom_operators: HashMap::new(),
            preserve_structure,
            config,
        }
    }

    /// Gets a reference to the current evaluation configuration.
    pub fn config(&self) -> &EvaluationConfig {
        &self.config
    }

    /// Returns whether structure preservation is enabled.
    pub fn preserve_structure(&self) -> bool {
        self.preserve_structure
    }

    /// Registers a custom operator with the engine.
    ///
    /// Custom operators extend the engine's functionality with domain-specific logic.
    /// They override built-in operators if the same name is used.
    ///
    /// # Arguments
    ///
    /// * `name` - The operator name (e.g., "custom_calc")
    /// * `operator` - The operator implementation
    ///
    /// # Example
    ///
    /// ```rust
    /// use datalogic_rs::{DataLogic, Operator, ContextStack, Evaluator, Result, Error};
    /// use serde_json::{json, Value};
    ///
    /// struct DoubleOperator;
    ///
    /// impl Operator for DoubleOperator {
    ///     fn evaluate(
    ///         &self,
    ///         args: &[Value],
    ///         _context: &mut ContextStack,
    ///         _evaluator: &dyn Evaluator,
    ///     ) -> Result<Value> {
    ///         if let Some(n) = args.first().and_then(|v| v.as_f64()) {
    ///             Ok(json!(n * 2.0))
    ///         } else {
    ///             Err(Error::InvalidArguments("Argument must be a number".to_string()))
    ///         }
    ///     }
    /// }
    ///
    /// let mut engine = DataLogic::new();
    /// engine.add_operator("double".to_string(), Box::new(DoubleOperator));
    /// ```
    pub fn add_operator(&mut self, name: String, operator: Box<dyn Operator>) {
        self.custom_operators.insert(name, operator);
    }

    /// Checks if a custom operator with the given name is registered.
    ///
    /// # Arguments
    ///
    /// * `name` - The operator name to check
    ///
    /// # Returns
    ///
    /// `true` if the operator exists, `false` otherwise.
    pub fn has_custom_operator(&self, name: &str) -> bool {
        self.custom_operators.contains_key(name)
    }

    /// Compiles a JSON logic expression into an optimized form.
    ///
    /// Compilation performs:
    /// - Static evaluation of constant expressions
    /// - OpCode assignment for built-in operators
    /// - Structure analysis for templating
    ///
    /// The returned `Arc<CompiledLogic>` can be safely shared across threads.
    ///
    /// # Arguments
    ///
    /// * `logic` - The JSON logic expression to compile
    ///
    /// # Returns
    ///
    /// An `Arc`-wrapped compiled logic structure, or an error if compilation fails.
    ///
    /// # Example
    ///
    /// ```rust
    /// use datalogic_rs::DataLogic;
    /// use serde_json::json;
    /// use std::sync::Arc;
    ///
    /// let engine = DataLogic::new();
    /// let logic = json!({"==": [1, 1]});
    /// let compiled: Arc<_> = engine.compile(&logic).unwrap();
    /// ```
    pub fn compile(&self, logic: &Value) -> Result<Arc<CompiledLogic>> {
        let compiled = CompiledLogic::compile_with_static_eval(logic, self)?;
        Ok(Arc::new(compiled))
    }

    /// Evaluates compiled logic with Arc-wrapped data.
    ///
    /// Use this method when you already have data in an `Arc` to avoid re-wrapping.
    /// For owned data, use `evaluate_owned` instead.
    ///
    /// # Arguments
    ///
    /// * `compiled` - The compiled logic to evaluate
    /// * `data` - The data context wrapped in an `Arc`
    ///
    /// # Returns
    ///
    /// The evaluation result, or an error if evaluation fails.
    pub fn evaluate(&self, compiled: &CompiledLogic, data: Arc<Value>) -> Result<Value> {
        let mut context = ContextStack::new(data);
        self.evaluate_node(&compiled.root, &mut context)
    }

    /// Evaluates compiled logic with owned data.
    ///
    /// This is a convenience method that wraps the data in an `Arc` before evaluation.
    /// If you already have Arc-wrapped data, use `evaluate` instead.
    ///
    /// # Arguments
    ///
    /// * `compiled` - The compiled logic to evaluate
    /// * `data` - The owned data context
    ///
    /// # Returns
    ///
    /// The evaluation result, or an error if evaluation fails.
    ///
    /// # Example
    ///
    /// ```rust
    /// use datalogic_rs::DataLogic;
    /// use serde_json::json;
    ///
    /// let engine = DataLogic::new();
    /// let logic = json!({"var": "name"});
    /// let compiled = engine.compile(&logic).unwrap();
    ///
    /// let data = json!({"name": "Alice"});
    /// let result = engine.evaluate_owned(&compiled, data).unwrap();
    /// assert_eq!(result, json!("Alice"));
    /// ```
    pub fn evaluate_owned(&self, compiled: &CompiledLogic, data: Value) -> Result<Value> {
        self.evaluate(compiled, Arc::new(data))
    }

    /// Convenience method for evaluating JSON strings directly.
    ///
    /// This method combines compilation and evaluation in a single call.
    /// For repeated evaluations, compile once and reuse the compiled logic.
    ///
    /// # Arguments
    ///
    /// * `logic` - JSON logic as a string
    /// * `data` - Data context as a JSON string
    ///
    /// # Returns
    ///
    /// The evaluation result, or an error if parsing or evaluation fails.
    ///
    /// # Example
    ///
    /// ```rust
    /// use datalogic_rs::DataLogic;
    ///
    /// let engine = DataLogic::new();
    /// let result = engine.evaluate_json(
    ///     r#"{"==": [{"var": "x"}, 5]}"#,
    ///     r#"{"x": 5}"#
    /// ).unwrap();
    /// assert_eq!(result, serde_json::json!(true));
    /// ```
    pub fn evaluate_json(&self, logic: &str, data: &str) -> Result<Value> {
        let logic_value: Value = serde_json::from_str(logic)?;
        let data_value: Value = serde_json::from_str(data)?;
        let data_arc = Arc::new(data_value);

        let compiled = self.compile(&logic_value)?;
        self.evaluate(&compiled, data_arc)
    }

    /// Evaluates a compiled node using OpCode dispatch.
    ///
    /// This is the core evaluation method that handles:
    /// - Static values
    /// - Arrays
    /// - Built-in operators (via OpCode)
    /// - Custom operators
    /// - Structured objects (in preserve mode)
    ///
    /// # Arguments
    ///
    /// * `node` - The compiled node to evaluate
    /// * `context` - The context stack containing data and metadata
    ///
    /// # Returns
    ///
    /// The evaluation result, or an error if evaluation fails.
    pub fn evaluate_node(&self, node: &CompiledNode, context: &mut ContextStack) -> Result<Value> {
        match node {
            CompiledNode::Value { value, .. } => Ok(value.clone()),

            CompiledNode::Array { nodes, .. } => {
                // Use SmallVec for common small array sizes to avoid heap allocation
                let mut results: SmallVec<[Value; 4]> = SmallVec::with_capacity(nodes.len());
                for node in nodes.iter() {
                    results.push(self.evaluate_node(node, context)?);
                }
                Ok(Value::Array(results.into_vec()))
            }

            CompiledNode::BuiltinOperator { opcode, args, .. } => {
                // Direct OpCode dispatch with CompiledNode args
                opcode.evaluate_direct(args, context, self)
            }

            CompiledNode::CustomOperator { name, args, .. } => {
                // Custom operators still use dynamic dispatch
                let operator = self
                    .custom_operators
                    .get(name)
                    .ok_or_else(|| Error::InvalidOperator(name.clone()))?;

                let arg_values: Vec<Value> = args.iter().map(node_to_value).collect();
                let evaluator = SimpleEvaluator::new(self);

                operator.evaluate(&arg_values, context, &evaluator)
            }

            CompiledNode::StructuredObject { fields, .. } => {
                let mut result = serde_json::Map::new();
                for (key, node) in fields {
                    let value = self.evaluate_node(node, context)?;
                    result.insert(key.clone(), value);
                }
                Ok(Value::Object(result))
            }
        }
    }
}

/// Convert a compiled node back to a JSON value (for custom operators)
fn node_to_value(node: &CompiledNode) -> Value {
    match node {
        CompiledNode::Value { value, .. } => value.clone(),
        CompiledNode::Array { nodes, .. } => {
            Value::Array(nodes.iter().map(node_to_value).collect())
        }
        CompiledNode::BuiltinOperator { opcode, args, .. } => {
            let mut obj = serde_json::Map::new();
            let args_value = if args.len() == 1 {
                node_to_value(&args[0])
            } else {
                Value::Array(args.iter().map(node_to_value).collect())
            };
            obj.insert(opcode.as_str().into(), args_value);
            Value::Object(obj)
        }
        CompiledNode::CustomOperator { name, args, .. } => {
            let mut obj = serde_json::Map::new();
            let args_value = if args.len() == 1 {
                node_to_value(&args[0])
            } else {
                Value::Array(args.iter().map(node_to_value).collect())
            };
            obj.insert(name.clone(), args_value);
            Value::Object(obj)
        }
        CompiledNode::StructuredObject { fields, .. } => {
            let mut obj = serde_json::Map::new();
            for (key, node) in fields {
                obj.insert(key.clone(), node_to_value(node));
            }
            Value::Object(obj)
        }
    }
}

/// Simple evaluator that compiles and evaluates without caching
struct SimpleEvaluator<'e> {
    engine: &'e DataLogic,
}

impl<'e> SimpleEvaluator<'e> {
    /// Create a new SimpleEvaluator
    fn new(engine: &'e DataLogic) -> Self {
        Self { engine }
    }
}

impl Evaluator for SimpleEvaluator<'_> {
    fn evaluate(&self, logic: &Value, context: &mut ContextStack) -> Result<Value> {
        // Compile and evaluate - compilation already handles simple values efficiently
        match logic {
            Value::Object(obj) if obj.len() == 1 => {
                let compiled = CompiledLogic::compile_with_static_eval(logic, self.engine)?;
                self.engine.evaluate_node(&compiled.root, context)
            }
            Value::Object(obj) if obj.len() > 1 && self.engine.preserve_structure => {
                // Multi-key object in preserve_structure mode
                let compiled = CompiledLogic::compile_with_static_eval(logic, self.engine)?;
                self.engine.evaluate_node(&compiled.root, context)
            }
            Value::Array(_) => {
                let compiled = CompiledLogic::compile_with_static_eval(logic, self.engine)?;
                self.engine.evaluate_node(&compiled.root, context)
            }
            _ => Ok(logic.clone()),
        }
    }
}
