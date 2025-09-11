use serde_json::Value;
use std::borrow::Cow;
use std::collections::HashMap;
use std::sync::Arc;

use crate::{CompiledLogic, CompiledNode, ContextStack, Error, Evaluator, Operator, Result};

/// Main DataLogic engine
pub struct DataLogic {
    operators: HashMap<String, Box<dyn Operator>>,
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
            operators: HashMap::new(),
        };
        engine.register_builtin_operators();
        engine
    }

    /// Register all built-in operators
    fn register_builtin_operators(&mut self) {
        use crate::operators::*;

        // Variable access
        self.operators
            .insert("var".to_string(), Box::new(VarOperator));
        self.operators
            .insert("val".to_string(), Box::new(ValOperator));

        // Comparison operators
        self.operators
            .insert("==".to_string(), Box::new(EqualsOperator { strict: false }));
        self.operators
            .insert("===".to_string(), Box::new(EqualsOperator { strict: true }));
        self.operators.insert(
            "!=".to_string(),
            Box::new(NotEqualsOperator { strict: false }),
        );
        self.operators.insert(
            "!==".to_string(),
            Box::new(NotEqualsOperator { strict: true }),
        );
        self.operators
            .insert(">".to_string(), Box::new(GreaterThanOperator));
        self.operators
            .insert(">=".to_string(), Box::new(GreaterThanEqualOperator));
        self.operators
            .insert("<".to_string(), Box::new(LessThanOperator));
        self.operators
            .insert("<=".to_string(), Box::new(LessThanEqualOperator));

        // Logical operators
        self.operators
            .insert("!".to_string(), Box::new(NotOperator));
        self.operators
            .insert("!!".to_string(), Box::new(DoubleNotOperator));
        self.operators
            .insert("and".to_string(), Box::new(AndOperator));
        self.operators
            .insert("or".to_string(), Box::new(OrOperator));

        // Control flow
        self.operators
            .insert("if".to_string(), Box::new(IfOperator));
        self.operators
            .insert("?:".to_string(), Box::new(TernaryOperator));

        // Arithmetic operators
        self.operators
            .insert("+".to_string(), Box::new(AddOperator));
        self.operators
            .insert("-".to_string(), Box::new(SubtractOperator));
        self.operators
            .insert("*".to_string(), Box::new(MultiplyOperator));
        self.operators
            .insert("/".to_string(), Box::new(DivideOperator));
        self.operators
            .insert("%".to_string(), Box::new(ModuloOperator));
        self.operators
            .insert("max".to_string(), Box::new(MaxOperator));
        self.operators
            .insert("min".to_string(), Box::new(MinOperator));

        // String operators
        self.operators
            .insert("cat".to_string(), Box::new(CatOperator));
        self.operators
            .insert("substr".to_string(), Box::new(SubstrOperator));
        self.operators
            .insert("in".to_string(), Box::new(InOperator));

        // Array operators
        self.operators
            .insert("merge".to_string(), Box::new(MergeOperator));
        self.operators
            .insert("filter".to_string(), Box::new(FilterOperator));
        self.operators
            .insert("map".to_string(), Box::new(MapOperator));
        self.operators
            .insert("reduce".to_string(), Box::new(ReduceOperator));
        self.operators
            .insert("all".to_string(), Box::new(AllOperator));
        self.operators
            .insert("some".to_string(), Box::new(SomeOperator));
        self.operators
            .insert("none".to_string(), Box::new(NoneOperator));

        // Missing operators
        self.operators
            .insert("missing".to_string(), Box::new(MissingOperator));
        self.operators
            .insert("missing_some".to_string(), Box::new(MissingSomeOperator));
    }

    /// Register a custom operator
    pub fn add_operator(&mut self, name: String, operator: Box<dyn Operator>) {
        self.operators.insert(name, operator);
    }

    /// Compile a logic expression
    pub fn compile(&self, logic: Cow<'_, Value>) -> Result<Arc<CompiledLogic>> {
        let compiled = CompiledLogic::compile(logic.as_ref())?;
        Ok(Arc::new(compiled))
    }

    /// Evaluate compiled logic with data
    pub fn evaluate<'a>(
        &self,
        compiled: &CompiledLogic,
        data: Cow<'a, Value>,
    ) -> Result<Cow<'a, Value>> {
        let mut context = ContextStack::new(data);
        self.evaluate_node(&compiled.root, &mut context)
    }

    /// Convenience method for owned values
    pub fn evaluate_owned(&self, compiled: &CompiledLogic, data: Value) -> Result<Value> {
        self.evaluate(compiled, Cow::Owned(data))
            .map(|cow| cow.into_owned())
    }

    /// Convenience method for borrowed values
    pub fn evaluate_ref<'a>(
        &self,
        compiled: &CompiledLogic,
        data: &'a Value,
    ) -> Result<Cow<'a, Value>> {
        self.evaluate(compiled, Cow::Borrowed(data))
    }

    /// Convenience method for JSON strings
    pub fn evaluate_json(&self, logic: &str, data: &str) -> Result<Value> {
        let logic_value: Value = serde_json::from_str(logic)?;
        let data_value: Value = serde_json::from_str(data)?;

        let compiled = self.compile(Cow::Borrowed(&logic_value))?;
        self.evaluate_owned(&compiled, data_value)
    }

    /// Evaluate a compiled node
    fn evaluate_node<'a>(
        &self,
        node: &CompiledNode,
        context: &mut ContextStack<'a>,
    ) -> Result<Cow<'a, Value>> {
        match node {
            CompiledNode::Value(val) => Ok(Cow::Owned(val.clone())),

            CompiledNode::Array(nodes) => {
                let mut results = Vec::with_capacity(nodes.len());
                for node in nodes {
                    results.push(self.evaluate_node(node, context)?.into_owned());
                }
                Ok(Cow::Owned(Value::Array(results)))
            }

            CompiledNode::Operator { name, args } => {
                // Look up the operator
                let operator = self
                    .operators
                    .get(name)
                    .ok_or_else(|| Error::InvalidOperator(name.clone()))?;

                // Prepare arguments as Cow values
                let arg_values: Vec<Cow<'_, Value>> = args
                    .iter()
                    .map(|arg| {
                        // Instead of evaluating here, pass the node as a Value
                        // The operator will evaluate if needed
                        match arg {
                            CompiledNode::Value(v) => Cow::Owned(v.clone()),
                            _ => {
                                // Convert node back to JSON for operator to evaluate
                                Cow::Owned(self.node_to_value(arg))
                            }
                        }
                    })
                    .collect();

                // Create an evaluator wrapper for this engine
                let evaluator = EngineEvaluator { engine: self };

                // Execute the operator
                operator.evaluate(&arg_values, context, &evaluator)
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
        CompiledNode::Operator { name, args } => {
            let mut obj = serde_json::Map::new();
            let args_value = if args.len() == 1 {
                node_to_value_impl(&args[0])
            } else {
                Value::Array(args.iter().map(node_to_value_impl).collect())
            };
            obj.insert(name.clone(), args_value);
            Value::Object(obj)
        }
    }
}

/// Evaluator implementation that wraps the engine
struct EngineEvaluator<'e> {
    engine: &'e DataLogic,
}

impl Evaluator for EngineEvaluator<'_> {
    fn evaluate<'a>(
        &self,
        logic: &Cow<'a, Value>,
        context: &mut ContextStack<'a>,
    ) -> Result<Cow<'a, Value>> {
        // Properly compile and evaluate the logic
        let compiled = CompiledLogic::compile(logic.as_ref())?;
        self.engine.evaluate_node(&compiled.root, context)
    }
}
