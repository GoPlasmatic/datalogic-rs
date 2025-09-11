use crate::{ContextStack, DataLogic, Result, opcode::OpCode};
use serde_json::{Value, json};

/// Compiled node representing a single operation or value
#[derive(Debug, Clone)]
pub enum CompiledNode {
    /// Static value
    Value(Value),

    /// Array of nodes
    Array(Box<[CompiledNode]>),

    /// Built-in operator with OpCode for fast lookup
    BuiltinOperator {
        opcode: OpCode,
        args: Vec<CompiledNode>,
    },

    /// Custom operator with string name
    CustomOperator {
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
        let root = Self::compile_node(logic, None)?;
        Ok(Self::new(root))
    }

    /// Compile with static evaluation using the provided engine
    pub fn compile_with_static_eval(logic: &Value, engine: &DataLogic) -> Result<Self> {
        let root = Self::compile_node(logic, Some(engine))?;
        Ok(Self::new(root))
    }

    /// Compile a single node
    fn compile_node(value: &Value, engine: Option<&DataLogic>) -> Result<CompiledNode> {
        match value {
            Value::Object(obj) if obj.len() > 1 => {
                // Multi-key objects are not valid operators
                Err(crate::error::Error::InvalidOperator(
                    "Unknown Operator".to_string(),
                ))
            }
            Value::Object(obj) if obj.len() == 1 => {
                // Single key object is an operator
                let (op_name, args_value) = obj.iter().next().unwrap();

                // Try to parse as built-in operator first
                if let Some(opcode) = OpCode::from_str(op_name) {
                    // Check if this operator requires array arguments
                    let requires_array = matches!(opcode, OpCode::And | OpCode::Or | OpCode::If);

                    // For operators that require arrays, check the raw value
                    if requires_array && !matches!(args_value, Value::Array(_)) {
                        // Create a special marker node for invalid arguments
                        let node = CompiledNode::BuiltinOperator {
                            opcode,
                            args: vec![CompiledNode::Value(json!({
                                "__invalid_args__": true,
                                "value": args_value
                            }))],
                        };
                        return Ok(node);
                    }

                    let args = Self::compile_args(args_value, engine)?;
                    let node = CompiledNode::BuiltinOperator { opcode, args };

                    // If engine is provided and node is static, evaluate it
                    if let std::option::Option::Some(eng) = engine
                        && Self::node_is_static(&node)
                    {
                        // Evaluate with empty context since it's static
                        let mut context = ContextStack::new(Value::Null);
                        match eng.evaluate_node(&node, &mut context) {
                            Ok(value) => return Ok(CompiledNode::Value(value)),
                            // If evaluation fails, keep as operator node
                            Err(_) => return Ok(node),
                        }
                    }

                    Ok(node)
                } else {
                    let args = Self::compile_args(args_value, engine)?;
                    // Fall back to custom operator - don't pre-evaluate custom operators
                    Ok(CompiledNode::CustomOperator {
                        name: op_name.clone(),
                        args,
                    })
                }
            }
            Value::Array(arr) => {
                // Array of logic expressions
                let nodes = arr
                    .iter()
                    .map(|v| Self::compile_node(v, engine))
                    .collect::<Result<Vec<_>>>()?;

                let node = CompiledNode::Array(nodes.into_boxed_slice());

                // If engine is provided and array is static, evaluate it
                if let std::option::Option::Some(eng) = engine
                    && Self::node_is_static(&node)
                {
                    let mut context = ContextStack::new(Value::Null);
                    if let Ok(value) = eng.evaluate_node(&node, &mut context) {
                        return Ok(CompiledNode::Value(value));
                    }
                }

                Ok(node)
            }
            _ => {
                // Static value
                Ok(CompiledNode::Value(value.clone()))
            }
        }
    }

    /// Compile operator arguments
    fn compile_args(value: &Value, engine: Option<&DataLogic>) -> Result<Vec<CompiledNode>> {
        match value {
            Value::Array(arr) => arr
                .iter()
                .map(|v| Self::compile_node(v, engine))
                .collect::<Result<Vec<_>>>(),
            _ => {
                // Single argument - compile it
                Ok(vec![Self::compile_node(value, engine)?])
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
            CompiledNode::BuiltinOperator { opcode, args } => {
                // Only certain operators can be static
                use OpCode::*;
                match opcode {
                    // These operators always depend on context
                    Var | Val | Missing | MissingSome => false,
                    // Array operations depend on their arguments
                    Map | Filter | Reduce | All | Some | None => false,
                    // Error handling operators may depend on context
                    Try | Throw => false,
                    // Type and string operators can be static if their args are
                    Type | StartsWith | EndsWith | Upper | Lower | Trim | Split => {
                        args.iter().all(Self::node_is_static)
                    }
                    // Datetime operators are static-ish
                    Datetime | Timestamp | ParseDate | FormatDate | DateDiff => {
                        args.iter().all(Self::node_is_static)
                    }
                    // Math operators are static if their args are
                    Abs | Ceil | Floor => args.iter().all(Self::node_is_static),
                    // Preserve should not be static - operators need to know it's from an operator
                    Preserve => false,
                    // These operators never depend on context
                    Add | Subtract | Multiply | Divide | Modulo | Min | Max | Equals
                    | StrictEquals | NotEquals | StrictNotEquals | GreaterThan
                    | GreaterThanEqual | LessThan | LessThanEqual | Not | DoubleNot | And | Or
                    | Ternary | If | Cat | Substr | In | Length | Sort | Slice => {
                        args.iter().all(Self::node_is_static)
                    }
                    // Merge is not statically evaluated because max/min need to distinguish
                    // between literal arrays and arrays from operators
                    Merge => false,
                    // Coalesce can be static if its args are
                    Coalesce => args.iter().all(Self::node_is_static),
                    // Exists depends on context
                    Exists => false,
                }
            }
            CompiledNode::CustomOperator { .. } => {
                // Unknown operators are assumed to be non-static
                false
            }
        }
    }
}
