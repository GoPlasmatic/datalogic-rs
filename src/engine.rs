use rustc_hash::FxHashMap;
use serde_json::Value;
use std::collections::HashMap;
use std::sync::Arc;

use crate::{CompiledLogic, CompiledNode, ContextStack, Error, Evaluator, Operator, Result};

/// Main DataLogic engine
pub struct DataLogic {
    // No more builtin_operators array - OpCode handles dispatch directly!
    /// HashMap for custom operators only
    custom_operators: HashMap<String, Box<dyn Operator>>,
    /// Flag to preserve structure of objects with unknown operators
    preserve_structure: bool,
}

impl Default for DataLogic {
    fn default() -> Self {
        Self::new()
    }
}

impl DataLogic {
    /// Create a new DataLogic engine with built-in operators
    pub fn new() -> Self {
        Self {
            custom_operators: HashMap::new(),
            preserve_structure: false,
        }
    }

    /// Create a new DataLogic engine with preserve_structure enabled
    pub fn with_preserve_structure() -> Self {
        Self {
            custom_operators: HashMap::new(),
            preserve_structure: true,
        }
    }

    /// Set the preserve_structure flag
    pub fn set_preserve_structure(&mut self, preserve: bool) {
        self.preserve_structure = preserve;
    }

    /// Get the preserve_structure flag
    pub fn preserve_structure(&self) -> bool {
        self.preserve_structure
    }

    /// Register a custom operator
    pub fn add_operator(&mut self, name: String, operator: Box<dyn Operator>) {
        self.custom_operators.insert(name, operator);
    }

    /// Compile a logic expression with static evaluation
    pub fn compile(&self, logic: &Value) -> Result<Arc<CompiledLogic>> {
        let compiled = CompiledLogic::compile_with_static_eval(logic, self)?;
        Ok(Arc::new(compiled))
    }

    /// Evaluate compiled logic with Arc data
    /// Use this when you already have data in an Arc to avoid re-wrapping
    pub fn evaluate(&self, compiled: &CompiledLogic, data: Arc<Value>) -> Result<Value> {
        let mut context = ContextStack::new(data);
        self.evaluate_node(&compiled.root, &mut context)
    }

    /// Convenience method for JSON strings
    pub fn evaluate_json(&self, logic: &str, data: &str) -> Result<Value> {
        let logic_value: Value = serde_json::from_str(logic)?;
        let data_value: Value = serde_json::from_str(data)?;
        let data_arc = Arc::new(data_value);

        let compiled = self.compile(&logic_value)?;
        self.evaluate(&compiled, data_arc)
    }

    /// Evaluate a compiled node using OpCode dispatch
    pub fn evaluate_node(&self, node: &CompiledNode, context: &mut ContextStack) -> Result<Value> {
        match node {
            CompiledNode::Value { value, .. } => Ok(value.clone()),

            CompiledNode::Array { nodes, .. } => {
                let mut results = Vec::with_capacity(nodes.len());
                for node in nodes.iter() {
                    results.push(self.evaluate_node(node, context)?);
                }
                Ok(Value::Array(results))
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

                let arg_values: Vec<Value> =
                    args.iter().map(|arg| self.node_to_value(arg)).collect();
                let evaluator = FastEvaluator::new(self, args);

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

    /// Convert a compiled node back to a JSON value (only for custom operators)
    fn node_to_value(&self, node: &CompiledNode) -> Value {
        node_to_value_impl(node)
    }
}

/// Convert a compiled node back to a JSON value (standalone helper)
fn node_to_value_impl(node: &CompiledNode) -> Value {
    match node {
        CompiledNode::Value { value, .. } => value.clone(),
        CompiledNode::Array { nodes, .. } => {
            Value::Array(nodes.iter().map(node_to_value_impl).collect())
        }
        CompiledNode::BuiltinOperator { opcode, args, .. } => {
            let mut obj = serde_json::Map::new();
            let args_value = if args.len() == 1 {
                node_to_value_impl(&args[0])
            } else {
                Value::Array(args.iter().map(node_to_value_impl).collect())
            };
            obj.insert(opcode.as_str().to_string(), args_value);
            Value::Object(obj)
        }
        CompiledNode::CustomOperator { name, args, .. } => {
            let mut obj = serde_json::Map::new();
            let args_value = if args.len() == 1 {
                node_to_value_impl(&args[0])
            } else {
                Value::Array(args.iter().map(node_to_value_impl).collect())
            };
            obj.insert(name.clone(), args_value);
            Value::Object(obj)
        }
        CompiledNode::StructuredObject { fields, .. } => {
            let mut obj = serde_json::Map::new();
            for (key, node) in fields {
                obj.insert(key.clone(), node_to_value_impl(node));
            }
            Value::Object(obj)
        }
    }
}

/// Fast evaluator that avoids recompilation
struct FastEvaluator<'e> {
    engine: &'e DataLogic,
    nodes: &'e [CompiledNode],
    // Hash map for O(1) node lookup
    node_map: FxHashMap<u64, usize>,
}

impl<'e> FastEvaluator<'e> {
    /// Create a new FastEvaluator with precomputed hash map
    fn new(engine: &'e DataLogic, nodes: &'e [CompiledNode]) -> Self {
        let mut node_map = FxHashMap::default();
        for (idx, node) in nodes.iter().enumerate() {
            node_map.insert(node.hash(), idx);
        }
        Self {
            engine,
            nodes,
            node_map,
        }
    }
}

impl Evaluator for FastEvaluator<'_> {
    fn evaluate(&self, logic: &Value, context: &mut ContextStack) -> Result<Value> {
        // Fast path: check if this is a simple value first
        match logic {
            Value::Null | Value::Bool(_) | Value::Number(_) | Value::String(_) => {
                return Ok(logic.clone());
            }
            _ => {}
        }

        // Use hash-based lookup for O(1) performance
        let logic_hash = crate::compiled::hash_value(logic);

        // Check if we have a pre-compiled node with this hash
        if let Some(&node_idx) = self.node_map.get(&logic_hash) {
            let node = &self.nodes[node_idx];

            // Verify the node actually matches (in case of hash collision)
            // This is a quick structural check, not a full reconstruction
            let matches = match (node, logic) {
                (CompiledNode::Value { value, .. }, val) => value == val,
                (CompiledNode::BuiltinOperator { .. }, Value::Object(obj)) if obj.len() == 1 => {
                    // For rare hash collisions, do full check
                    let node_val = node_to_value_impl(node);
                    &node_val == logic
                }
                (CompiledNode::CustomOperator { .. }, Value::Object(obj)) if obj.len() == 1 => {
                    let node_val = node_to_value_impl(node);
                    &node_val == logic
                }
                (CompiledNode::StructuredObject { .. }, Value::Object(_)) => {
                    let node_val = node_to_value_impl(node);
                    &node_val == logic
                }
                (CompiledNode::Array { .. }, Value::Array(_)) => {
                    let node_val = node_to_value_impl(node);
                    &node_val == logic
                }
                _ => false,
            };

            if matches {
                return self.engine.evaluate_node(node, context);
            }
        }

        // Fallback: compile and evaluate with proper preserve_structure setting
        match logic {
            Value::Object(obj) if obj.len() == 1 => {
                // Use compile_with_static_eval to respect preserve_structure flag
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
