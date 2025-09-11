use crate::Result;
use serde_json::Value;

/// Compiled node representing a single operation or value
#[derive(Debug, Clone)]
pub enum CompiledNode {
    /// Static value
    Value(Value),

    /// Array of nodes
    Array(Vec<CompiledNode>),

    /// Operator with its arguments
    Operator {
        name: String,
        args: Vec<CompiledNode>,
    },
}

/// Compiled logic that can be evaluated multiple times
#[derive(Debug, Clone)]
pub struct CompiledLogic {
    pub root: CompiledNode,
}

impl CompiledLogic {
    /// Create a new compiled logic from a root node
    pub fn new(root: CompiledNode) -> Self {
        Self { root }
    }

    /// Compile a JSON value into a compiled logic structure
    pub fn compile(logic: &Value) -> Result<Self> {
        let root = Self::compile_node(logic)?;
        Ok(Self::new(root))
    }

    /// Compile a single node
    fn compile_node(value: &Value) -> Result<CompiledNode> {
        match value {
            Value::Object(obj) if obj.len() == 1 => {
                // Single key object is an operator
                let (op_name, args_value) = obj.iter().next().unwrap();
                let args = Self::compile_args(args_value)?;
                Ok(CompiledNode::Operator {
                    name: op_name.clone(),
                    args,
                })
            }
            Value::Array(arr) => {
                // Array of logic expressions
                let nodes = arr
                    .iter()
                    .map(Self::compile_node)
                    .collect::<Result<Vec<_>>>()?;
                Ok(CompiledNode::Array(nodes))
            }
            _ => {
                // Static value
                Ok(CompiledNode::Value(value.clone()))
            }
        }
    }

    /// Compile operator arguments
    fn compile_args(value: &Value) -> Result<Vec<CompiledNode>> {
        match value {
            Value::Array(arr) => arr
                .iter()
                .map(Self::compile_node)
                .collect::<Result<Vec<_>>>(),
            _ => {
                // Single argument
                Ok(vec![Self::compile_node(value)?])
            }
        }
    }

    /// Check if this compiled logic is static (can be evaluated without context)
    pub fn is_static(&self) -> bool {
        Self::node_is_static(&self.root)
    }

    fn node_is_static(node: &CompiledNode) -> bool {
        match node {
            CompiledNode::Value(_) => true,
            CompiledNode::Array(nodes) => nodes.iter().all(Self::node_is_static),
            CompiledNode::Operator { name, args } => {
                // Only certain operators can be static
                match name.as_str() {
                    // These operators never depend on context
                    "+" | "-" | "*" | "/" | "%" | "min" | "max" | "==" | "===" | "!=" | "!=="
                    | ">" | ">=" | "<" | "<=" | "!" | "!!" | "and" | "or" | "?:" | "if" | "cat"
                    | "substr" | "in" | "merge" => args.iter().all(Self::node_is_static),
                    // These operators always depend on context
                    "var" | "val" | "missing" | "missing_some" => false,
                    // Array operations depend on their arguments
                    "map" | "filter" | "reduce" | "all" | "some" | "none" => false,
                    // Unknown operators are assumed to be non-static
                    _ => false,
                }
            }
        }
    }
}
