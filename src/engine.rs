use serde_json::Value;
use std::collections::HashMap;
use std::sync::Arc;

use crate::opcode::OpCode;
use crate::{CompiledLogic, CompiledNode, ContextStack, Error, Evaluator, Operator, Result};

/// Main DataLogic engine
pub struct DataLogic {
    /// Array for built-in operators (fast lookup)
    builtin_operators: [Option<Box<dyn Operator>>; OpCode::COUNT],
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
        let mut engine = Self {
            builtin_operators: std::array::from_fn(|_| None),
            custom_operators: HashMap::new(),
            preserve_structure: false,
        };
        engine.register_builtin_operators();
        engine
    }

    /// Create a new DataLogic engine with preserve_structure enabled
    pub fn with_preserve_structure() -> Self {
        let mut engine = Self {
            builtin_operators: std::array::from_fn(|_| None),
            custom_operators: HashMap::new(),
            preserve_structure: true,
        };
        engine.register_builtin_operators();
        engine
    }

    /// Set the preserve_structure flag
    pub fn set_preserve_structure(&mut self, preserve: bool) {
        self.preserve_structure = preserve;
    }

    /// Get the preserve_structure flag
    pub fn preserve_structure(&self) -> bool {
        self.preserve_structure
    }

    /// Register all built-in operators
    fn register_builtin_operators(&mut self) {
        use crate::operators::*;

        // Variable access
        self.builtin_operators[OpCode::Var as usize] = Some(Box::new(VarOperator));
        self.builtin_operators[OpCode::Val as usize] = Some(Box::new(ValOperator));
        self.builtin_operators[OpCode::Exists as usize] = Some(Box::new(ExistsOperator));

        // Comparison operators
        self.builtin_operators[OpCode::Equals as usize] =
            Some(Box::new(EqualsOperator { strict: false }));
        self.builtin_operators[OpCode::StrictEquals as usize] =
            Some(Box::new(EqualsOperator { strict: true }));
        self.builtin_operators[OpCode::NotEquals as usize] =
            Some(Box::new(NotEqualsOperator { strict: false }));
        self.builtin_operators[OpCode::StrictNotEquals as usize] =
            Some(Box::new(NotEqualsOperator { strict: true }));
        self.builtin_operators[OpCode::GreaterThan as usize] = Some(Box::new(GreaterThanOperator));
        self.builtin_operators[OpCode::GreaterThanEqual as usize] =
            Some(Box::new(GreaterThanEqualOperator));
        self.builtin_operators[OpCode::LessThan as usize] = Some(Box::new(LessThanOperator));
        self.builtin_operators[OpCode::LessThanEqual as usize] =
            Some(Box::new(LessThanEqualOperator));

        // Logical operators
        self.builtin_operators[OpCode::Not as usize] = Some(Box::new(NotOperator));
        self.builtin_operators[OpCode::DoubleNot as usize] = Some(Box::new(DoubleNotOperator));
        self.builtin_operators[OpCode::And as usize] = Some(Box::new(AndOperator));
        self.builtin_operators[OpCode::Or as usize] = Some(Box::new(OrOperator));

        // Control flow
        self.builtin_operators[OpCode::If as usize] = Some(Box::new(IfOperator));
        self.builtin_operators[OpCode::Ternary as usize] = Some(Box::new(TernaryOperator));
        self.builtin_operators[OpCode::Coalesce as usize] = Some(Box::new(CoalesceOperator));

        // Arithmetic operators
        self.builtin_operators[OpCode::Add as usize] = Some(Box::new(AddOperator));
        self.builtin_operators[OpCode::Subtract as usize] = Some(Box::new(SubtractOperator));
        self.builtin_operators[OpCode::Multiply as usize] = Some(Box::new(MultiplyOperator));
        self.builtin_operators[OpCode::Divide as usize] = Some(Box::new(DivideOperator));
        self.builtin_operators[OpCode::Modulo as usize] = Some(Box::new(ModuloOperator));
        self.builtin_operators[OpCode::Max as usize] = Some(Box::new(MaxOperator));
        self.builtin_operators[OpCode::Min as usize] = Some(Box::new(MinOperator));

        // String operators
        self.builtin_operators[OpCode::Cat as usize] = Some(Box::new(CatOperator));
        self.builtin_operators[OpCode::Substr as usize] = Some(Box::new(SubstrOperator));
        self.builtin_operators[OpCode::In as usize] = Some(Box::new(InOperator));
        self.builtin_operators[OpCode::Length as usize] = Some(Box::new(LengthOperator));

        // Array operators
        self.builtin_operators[OpCode::Merge as usize] = Some(Box::new(MergeOperator));
        self.builtin_operators[OpCode::Filter as usize] = Some(Box::new(FilterOperator));
        self.builtin_operators[OpCode::Map as usize] = Some(Box::new(MapOperator));
        self.builtin_operators[OpCode::Reduce as usize] = Some(Box::new(ReduceOperator));
        self.builtin_operators[OpCode::All as usize] = Some(Box::new(AllOperator));
        self.builtin_operators[OpCode::Some as usize] = Some(Box::new(SomeOperator));
        self.builtin_operators[OpCode::None as usize] = Some(Box::new(NoneOperator));
        self.builtin_operators[OpCode::Sort as usize] = Some(Box::new(SortOperator));
        self.builtin_operators[OpCode::Slice as usize] = Some(Box::new(SliceOperator));

        // Missing operators
        self.builtin_operators[OpCode::Missing as usize] = Some(Box::new(MissingOperator));
        self.builtin_operators[OpCode::MissingSome as usize] = Some(Box::new(MissingSomeOperator));

        // Error handling operators
        self.builtin_operators[OpCode::Try as usize] = Some(Box::new(TryOperator));
        self.builtin_operators[OpCode::Throw as usize] = Some(Box::new(ThrowOperator));

        // Type operator
        self.builtin_operators[OpCode::Type as usize] = Some(Box::new(TypeOperator));

        // String operators
        self.builtin_operators[OpCode::StartsWith as usize] = Some(Box::new(StartsWithOperator));
        self.builtin_operators[OpCode::EndsWith as usize] = Some(Box::new(EndsWithOperator));
        self.builtin_operators[OpCode::Upper as usize] = Some(Box::new(UpperOperator));
        self.builtin_operators[OpCode::Lower as usize] = Some(Box::new(LowerOperator));
        self.builtin_operators[OpCode::Trim as usize] = Some(Box::new(TrimOperator));
        self.builtin_operators[OpCode::Split as usize] = Some(Box::new(SplitOperator));

        // Datetime operators
        self.builtin_operators[OpCode::Datetime as usize] = Some(Box::new(DatetimeOperator));
        self.builtin_operators[OpCode::Timestamp as usize] = Some(Box::new(TimestampOperator));
        self.builtin_operators[OpCode::ParseDate as usize] = Some(Box::new(ParseDateOperator));
        self.builtin_operators[OpCode::FormatDate as usize] = Some(Box::new(FormatDateOperator));
        self.builtin_operators[OpCode::DateDiff as usize] = Some(Box::new(DateDiffOperator));
        self.builtin_operators[OpCode::Now as usize] = Some(Box::new(NowOperator));

        // Math operators
        self.builtin_operators[OpCode::Abs as usize] = Some(Box::new(AbsOperator));
        self.builtin_operators[OpCode::Ceil as usize] = Some(Box::new(CeilOperator));
        self.builtin_operators[OpCode::Floor as usize] = Some(Box::new(FloorOperator));

        // Utility operators
        self.builtin_operators[OpCode::Preserve as usize] = Some(Box::new(PreserveOperator));
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

    /// Evaluate a compiled node
    pub fn evaluate_node(&self, node: &CompiledNode, context: &mut ContextStack) -> Result<Value> {
        match node {
            CompiledNode::Value(val) => Ok(val.clone()),

            CompiledNode::Array(nodes) => {
                let mut results = Vec::with_capacity(nodes.len());
                for node in nodes {
                    results.push(self.evaluate_node(node, context)?);
                }
                Ok(Value::Array(results))
            }

            CompiledNode::BuiltinOperator { opcode, args } => {
                // Direct array access - super fast!
                let operator = self.builtin_operators[*opcode as usize]
                    .as_ref()
                    .expect("Built-in operator not found");

                // Prepare arguments as Values - don't evaluate yet
                let arg_values: Vec<Value> =
                    args.iter().map(|arg| self.node_to_value(arg)).collect();

                // Create an evaluator wrapper for this engine with cached compiled nodes
                let evaluator = FastEvaluator {
                    engine: self,
                    nodes: args,
                };

                // Execute the operator
                operator.evaluate(&arg_values, context, &evaluator)
            }

            CompiledNode::CustomOperator { name, args } => {
                // HashMap lookup only for custom operators
                let operator = self
                    .custom_operators
                    .get(name)
                    .ok_or_else(|| Error::InvalidOperator(name.clone()))?;

                // Prepare arguments as Values - don't evaluate yet
                let arg_values: Vec<Value> =
                    args.iter().map(|arg| self.node_to_value(arg)).collect();

                // Create an evaluator wrapper for this engine with cached compiled nodes
                let evaluator = FastEvaluator {
                    engine: self,
                    nodes: args,
                };

                // Execute the operator
                operator.evaluate(&arg_values, context, &evaluator)
            }

            CompiledNode::StructuredObject(fields) => {
                // Evaluate each field independently and build the result object
                let mut result = serde_json::Map::new();
                for (key, node) in fields {
                    let value = self.evaluate_node(node, context)?;
                    result.insert(key.clone(), value);
                }
                Ok(Value::Object(result))
            }
        }
    }

    /// Convert a compiled node back to a JSON value (for passing to operators)
    fn node_to_value(&self, node: &CompiledNode) -> Value {
        node_to_value_impl(node)
    }
}

/// Convert a compiled node back to a JSON value (standalone helper)
fn node_to_value_impl(node: &CompiledNode) -> Value {
    match node {
        CompiledNode::Value(val) => val.clone(),
        CompiledNode::Array(nodes) => Value::Array(nodes.iter().map(node_to_value_impl).collect()),
        CompiledNode::BuiltinOperator { opcode, args } => {
            let mut obj = serde_json::Map::new();
            let args_value = if args.len() == 1 {
                node_to_value_impl(&args[0])
            } else {
                Value::Array(args.iter().map(node_to_value_impl).collect())
            };
            obj.insert(opcode.as_str().to_string(), args_value);
            Value::Object(obj)
        }
        CompiledNode::CustomOperator { name, args } => {
            let mut obj = serde_json::Map::new();
            let args_value = if args.len() == 1 {
                node_to_value_impl(&args[0])
            } else {
                Value::Array(args.iter().map(node_to_value_impl).collect())
            };
            obj.insert(name.clone(), args_value);
            Value::Object(obj)
        }
        CompiledNode::StructuredObject(fields) => {
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

        // Try to find the corresponding pre-compiled node
        // This avoids recompilation of operator arguments
        for node in self.nodes.iter() {
            // Quick check if this could be the right node
            match (node, logic) {
                (CompiledNode::Value(v), val) if v == val => {
                    return self.engine.evaluate_node(node, context);
                }
                (CompiledNode::BuiltinOperator { .. }, Value::Object(obj)) if obj.len() == 1 => {
                    // Check if this operator matches
                    let node_val = node_to_value_impl(node);
                    if &node_val == logic {
                        return self.engine.evaluate_node(node, context);
                    }
                }
                (CompiledNode::CustomOperator { .. }, Value::Object(obj)) if obj.len() == 1 => {
                    // Check if this operator matches
                    let node_val = node_to_value_impl(node);
                    if &node_val == logic {
                        return self.engine.evaluate_node(node, context);
                    }
                }
                (CompiledNode::Array(_), Value::Array(_)) => {
                    let node_val = node_to_value_impl(node);
                    if &node_val == logic {
                        return self.engine.evaluate_node(node, context);
                    }
                }
                _ => {}
            }
        }

        // Fallback: compile and evaluate
        match logic {
            Value::Object(obj) if obj.len() == 1 => {
                let compiled = CompiledLogic::compile(logic)?;
                self.engine.evaluate_node(&compiled.root, context)
            }
            Value::Array(_) => {
                let compiled = CompiledLogic::compile(logic)?;
                self.engine.evaluate_node(&compiled.root, context)
            }
            _ => Ok(logic.clone()),
        }
    }
}
