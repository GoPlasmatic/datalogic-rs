use crate::{ContextStack, DataLogic, Result, opcode::OpCode};
use rustc_hash::FxHasher;
use serde_json::{Value, json};
use std::hash::{Hash, Hasher};
use std::sync::Arc;

/// Compiled node representing a single operation or value
#[derive(Debug, Clone)]
pub enum CompiledNode {
    /// Static value with precomputed hash
    Value { value: Value, hash: u64 },

    /// Array of nodes with precomputed hash
    Array {
        nodes: Box<[CompiledNode]>,
        hash: u64,
    },

    /// Built-in operator with OpCode for fast lookup
    BuiltinOperator {
        opcode: OpCode,
        args: Vec<CompiledNode>,
        hash: u64,
    },

    /// Custom operator with string name
    CustomOperator {
        name: String,
        args: Vec<CompiledNode>,
        hash: u64,
    },

    /// Structured object (for preserve_structure mode)
    StructuredObject {
        fields: Vec<(String, CompiledNode)>,
        hash: u64,
    },
}

impl CompiledNode {
    /// Get the precomputed hash of this node
    pub fn hash(&self) -> u64 {
        match self {
            CompiledNode::Value { hash, .. } => *hash,
            CompiledNode::Array { hash, .. } => *hash,
            CompiledNode::BuiltinOperator { hash, .. } => *hash,
            CompiledNode::CustomOperator { hash, .. } => *hash,
            CompiledNode::StructuredObject { hash, .. } => *hash,
        }
    }
}

/// Hash a JSON value for fast comparison
pub fn hash_value(value: &Value) -> u64 {
    let mut hasher = FxHasher::default();

    match value {
        Value::Null => {
            0u8.hash(&mut hasher);
        }
        Value::Bool(b) => {
            1u8.hash(&mut hasher);
            b.hash(&mut hasher);
        }
        Value::Number(n) => {
            2u8.hash(&mut hasher);
            // Hash the string representation to ensure consistency
            n.to_string().hash(&mut hasher);
        }
        Value::String(s) => {
            3u8.hash(&mut hasher);
            s.hash(&mut hasher);
        }
        Value::Array(arr) => {
            4u8.hash(&mut hasher);
            arr.len().hash(&mut hasher);
            for v in arr {
                hash_value(v).hash(&mut hasher);
            }
        }
        Value::Object(obj) => {
            5u8.hash(&mut hasher);
            obj.len().hash(&mut hasher);
            // Sort keys to ensure consistent hashing
            let mut keys: Vec<_> = obj.keys().collect();
            keys.sort();
            for key in keys {
                key.hash(&mut hasher);
                hash_value(&obj[key]).hash(&mut hasher);
            }
        }
    }

    hasher.finish()
}

/// Compute hash for a compiled node
fn compute_node_hash(node: &CompiledNode) -> u64 {
    let mut hasher = FxHasher::default();

    match node {
        CompiledNode::Value { value, .. } => {
            // Hash type discriminant and value
            0u8.hash(&mut hasher);
            hash_value(value).hash(&mut hasher);
        }
        CompiledNode::Array { nodes, .. } => {
            1u8.hash(&mut hasher);
            nodes.len().hash(&mut hasher);
            for n in nodes.iter() {
                n.hash().hash(&mut hasher);
            }
        }
        CompiledNode::BuiltinOperator { opcode, args, .. } => {
            2u8.hash(&mut hasher);
            (*opcode as u8).hash(&mut hasher);
            args.len().hash(&mut hasher);
            for arg in args {
                arg.hash().hash(&mut hasher);
            }
        }
        CompiledNode::CustomOperator { name, args, .. } => {
            3u8.hash(&mut hasher);
            name.hash(&mut hasher);
            args.len().hash(&mut hasher);
            for arg in args {
                arg.hash().hash(&mut hasher);
            }
        }
        CompiledNode::StructuredObject { fields, .. } => {
            4u8.hash(&mut hasher);
            fields.len().hash(&mut hasher);
            for (key, node) in fields {
                key.hash(&mut hasher);
                node.hash().hash(&mut hasher);
            }
        }
    }

    hasher.finish()
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
        let root = Self::compile_node(logic, None, false)?;
        Ok(Self::new(root))
    }

    /// Compile with static evaluation using the provided engine
    pub fn compile_with_static_eval(logic: &Value, engine: &DataLogic) -> Result<Self> {
        let root = Self::compile_node(logic, Some(engine), engine.preserve_structure())?;
        Ok(Self::new(root))
    }

    /// Compile a single node
    fn compile_node(
        value: &Value,
        engine: Option<&DataLogic>,
        preserve_structure: bool,
    ) -> Result<CompiledNode> {
        match value {
            Value::Object(obj) if obj.len() > 1 => {
                if preserve_structure {
                    // In preserve_structure mode, treat multi-key objects as structured objects
                    // We'll create a special StructuredObject node that gets evaluated field by field
                    let mut fields = Vec::new();
                    for (key, val) in obj.iter() {
                        let compiled_val = Self::compile_node(val, engine, preserve_structure)?;
                        fields.push((key.clone(), compiled_val));
                    }
                    let mut node = CompiledNode::StructuredObject { fields, hash: 0 };
                    let hash = compute_node_hash(&node);
                    if let CompiledNode::StructuredObject { hash: h, .. } = &mut node {
                        *h = hash;
                    }
                    Ok(node)
                } else {
                    // Multi-key objects are not valid operators
                    Err(crate::error::Error::InvalidOperator(
                        "Unknown Operator".to_string(),
                    ))
                }
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
                        let invalid_value = json!({
                            "__invalid_args__": true,
                            "value": args_value
                        });
                        let value_hash = hash_value(&invalid_value);
                        let value_node = CompiledNode::Value {
                            value: invalid_value,
                            hash: value_hash,
                        };
                        let args = vec![value_node];
                        let mut node = CompiledNode::BuiltinOperator {
                            opcode,
                            args,
                            hash: 0,
                        };
                        let hash = compute_node_hash(&node);
                        if let CompiledNode::BuiltinOperator { hash: h, .. } = &mut node {
                            *h = hash;
                        }
                        return Ok(node);
                    }

                    // Special handling for preserve operator - don't compile its arguments
                    let args = if opcode == OpCode::Preserve {
                        // Preserve takes raw values, not compiled logic
                        match args_value {
                            Value::Array(arr) => arr
                                .iter()
                                .map(|v| {
                                    let value = v.clone();
                                    let hash = hash_value(&value);
                                    CompiledNode::Value { value, hash }
                                })
                                .collect(),
                            _ => {
                                let value = args_value.clone();
                                let hash = hash_value(&value);
                                vec![CompiledNode::Value { value, hash }]
                            }
                        }
                    } else {
                        Self::compile_args(args_value, engine, preserve_structure)?
                    };
                    let mut node = CompiledNode::BuiltinOperator {
                        opcode,
                        args,
                        hash: 0,
                    };
                    let hash = compute_node_hash(&node);
                    if let CompiledNode::BuiltinOperator { hash: h, .. } = &mut node {
                        *h = hash;
                    }

                    // If engine is provided and node is static, evaluate it
                    if let std::option::Option::Some(eng) = engine
                        && Self::node_is_static(&node)
                    {
                        // Evaluate with empty context since it's static
                        let mut context = ContextStack::new(Arc::new(Value::Null));
                        match eng.evaluate_node(&node, &mut context) {
                            Ok(value) => {
                                let hash = hash_value(&value);
                                return Ok(CompiledNode::Value { value, hash });
                            }
                            // If evaluation fails, keep as operator node
                            Err(_) => return Ok(node),
                        }
                    }

                    Ok(node)
                } else if preserve_structure {
                    // In preserve_structure mode, treat unknown operators as object keys
                    let compiled_val = Self::compile_node(args_value, engine, preserve_structure)?;
                    let fields = vec![(op_name.clone(), compiled_val)];
                    let mut node = CompiledNode::StructuredObject { fields, hash: 0 };
                    let hash = compute_node_hash(&node);
                    if let CompiledNode::StructuredObject { hash: h, .. } = &mut node {
                        *h = hash;
                    }
                    Ok(node)
                } else {
                    let args = Self::compile_args(args_value, engine, preserve_structure)?;
                    // Fall back to custom operator - don't pre-evaluate custom operators
                    let mut node = CompiledNode::CustomOperator {
                        name: op_name.clone(),
                        args,
                        hash: 0,
                    };
                    let hash = compute_node_hash(&node);
                    if let CompiledNode::CustomOperator { hash: h, .. } = &mut node {
                        *h = hash;
                    }
                    Ok(node)
                }
            }
            Value::Array(arr) => {
                // Array of logic expressions
                let nodes = arr
                    .iter()
                    .map(|v| Self::compile_node(v, engine, preserve_structure))
                    .collect::<Result<Vec<_>>>()?;

                let nodes_boxed = nodes.into_boxed_slice();
                let mut node = CompiledNode::Array {
                    nodes: nodes_boxed,
                    hash: 0,
                };
                let hash = compute_node_hash(&node);
                if let CompiledNode::Array { hash: h, .. } = &mut node {
                    *h = hash;
                }

                // If engine is provided and array is static, evaluate it
                if let std::option::Option::Some(eng) = engine
                    && Self::node_is_static(&node)
                {
                    let mut context = ContextStack::new(Arc::new(Value::Null));
                    if let Ok(value) = eng.evaluate_node(&node, &mut context) {
                        let hash = hash_value(&value);
                        return Ok(CompiledNode::Value { value, hash });
                    }
                }

                Ok(node)
            }
            _ => {
                // Static value
                let value = value.clone();
                let hash = hash_value(&value);
                Ok(CompiledNode::Value { value, hash })
            }
        }
    }

    /// Compile operator arguments
    fn compile_args(
        value: &Value,
        engine: Option<&DataLogic>,
        preserve_structure: bool,
    ) -> Result<Vec<CompiledNode>> {
        match value {
            Value::Array(arr) => arr
                .iter()
                .map(|v| Self::compile_node(v, engine, preserve_structure))
                .collect::<Result<Vec<_>>>(),
            _ => {
                // Single argument - compile it
                Ok(vec![Self::compile_node(value, engine, preserve_structure)?])
            }
        }
    }

    /// Check if this compiled logic is static (can be evaluated without context)
    pub fn is_static(&self) -> bool {
        Self::node_is_static(&self.root)
    }

    fn node_is_static(node: &CompiledNode) -> bool {
        match node {
            CompiledNode::Value { .. } => true,
            CompiledNode::Array { nodes, .. } => nodes.iter().all(Self::node_is_static),
            CompiledNode::BuiltinOperator { opcode, args, .. } => {
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
                    // Datetime operators are static-ish (except Now which always returns current time)
                    Datetime | Timestamp | ParseDate | FormatDate | DateDiff => {
                        args.iter().all(Self::node_is_static)
                    }
                    // Now always returns current time, so it's never static
                    Now => false,
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
            CompiledNode::StructuredObject { fields, .. } => {
                // Structured objects are static if all their fields are static
                fields.iter().all(|(_, node)| Self::node_is_static(node))
            }
        }
    }
}
