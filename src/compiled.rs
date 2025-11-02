use crate::{ContextStack, DataLogic, Result, opcode::OpCode};
use serde_json::{Value, json};
use std::sync::Arc;

/// A compiled node representing a single operation or value in the logic tree.
///
/// Nodes are created during the compilation phase and evaluated during execution.
/// Each node type is optimized for its specific purpose:
///
/// - **Value**: Static JSON values that don't require evaluation
/// - **Array**: Collections of nodes evaluated sequentially
/// - **BuiltinOperator**: Fast OpCode-based dispatch for built-in operators
/// - **CustomOperator**: User-defined operators with dynamic dispatch
/// - **StructuredObject**: Template objects for structure preservation
#[derive(Debug, Clone)]
pub enum CompiledNode {
    /// A static JSON value that requires no evaluation.
    ///
    /// Used for literals like numbers, strings, booleans, and null.
    Value { value: Value },

    /// An array of compiled nodes.
    ///
    /// Each node is evaluated in sequence, and the results are collected into a JSON array.
    /// Uses `Box<[CompiledNode]>` for memory efficiency.
    Array { nodes: Box<[CompiledNode]> },

    /// A built-in operator optimized with OpCode dispatch.
    ///
    /// The OpCode enum enables direct dispatch without string lookups,
    /// significantly improving performance for the 50+ built-in operators.
    BuiltinOperator {
        opcode: OpCode,
        args: Vec<CompiledNode>,
    },

    /// A custom operator registered via `DataLogic::add_operator`.
    ///
    /// Custom operators use dynamic dispatch and are looked up by name
    /// from the engine's operator registry.
    CustomOperator {
        name: String,
        args: Vec<CompiledNode>,
    },

    /// A structured object template for preserve_structure mode.
    ///
    /// When structure preservation is enabled, objects with keys that are not
    /// built-in operators or registered custom operators are preserved as templates.
    /// Each field is evaluated independently, allowing for dynamic object generation.
    ///
    /// Note: Custom operators are checked before treating keys as structured fields,
    /// ensuring they work correctly within preserved structures.
    StructuredObject { fields: Vec<(String, CompiledNode)> },
}

// Hash methods removed - no longer needed

// Hash functions removed - no longer needed

/// Compiled logic that can be evaluated multiple times across different data.
///
/// `CompiledLogic` represents a pre-processed JSONLogic expression that has been
/// optimized for repeated evaluation. It's thread-safe and can be shared across
/// threads using `Arc`.
///
/// # Performance Benefits
///
/// - **Parse once, evaluate many**: Avoid repeated JSON parsing
/// - **Static evaluation**: Constant expressions are pre-computed
/// - **OpCode dispatch**: Built-in operators use fast enum dispatch
/// - **Thread-safe sharing**: Use `Arc` to share across threads
///
/// # Example
///
/// ```rust
/// use datalogic_rs::DataLogic;
/// use serde_json::json;
/// use std::sync::Arc;
///
/// let engine = DataLogic::new();
/// let logic = json!({">": [{"var": "score"}, 90]});
/// let compiled = engine.compile(&logic).unwrap(); // Returns Arc<CompiledLogic>
///
/// // Can be shared across threads
/// let compiled_clone = Arc::clone(&compiled);
/// std::thread::spawn(move || {
///     let data = json!({"score": 95});
///     let result = engine.evaluate_owned(&compiled_clone, data);
/// });
/// ```
#[derive(Debug, Clone)]
pub struct CompiledLogic {
    /// The root node of the compiled logic tree
    pub root: CompiledNode,
}

impl CompiledLogic {
    /// Creates a new compiled logic from a root node.
    ///
    /// # Arguments
    ///
    /// * `root` - The root node of the compiled logic tree
    pub fn new(root: CompiledNode) -> Self {
        Self { root }
    }

    /// Compiles a JSON value into a compiled logic structure.
    ///
    /// This method performs basic compilation without static evaluation.
    /// For optimal performance, use `compile_with_static_eval` instead.
    ///
    /// # Arguments
    ///
    /// * `logic` - The JSON logic expression to compile
    ///
    /// # Returns
    ///
    /// A compiled logic structure, or an error if compilation fails.
    pub fn compile(logic: &Value) -> Result<Self> {
        let root = Self::compile_node(logic, None, false)?;
        Ok(Self::new(root))
    }

    /// Compiles with static evaluation using the provided engine.
    ///
    /// This method performs optimizations including:
    /// - Static evaluation of constant expressions
    /// - OpCode assignment for built-in operators
    /// - Structure preservation based on engine settings
    ///
    /// # Arguments
    ///
    /// * `logic` - The JSON logic expression to compile
    /// * `engine` - The DataLogic engine for static evaluation
    ///
    /// # Returns
    ///
    /// An optimized compiled logic structure, or an error if compilation fails.
    pub fn compile_with_static_eval(logic: &Value, engine: &DataLogic) -> Result<Self> {
        let root = Self::compile_node(logic, Some(engine), engine.preserve_structure())?;
        Ok(Self::new(root))
    }

    /// Compiles a single JSON value into a CompiledNode.
    ///
    /// This recursive method handles all node types:
    /// - Objects with operators
    /// - Arrays
    /// - Primitive values
    /// - Structured objects (in preserve mode)
    ///
    /// # Arguments
    ///
    /// * `value` - The JSON value to compile
    /// * `engine` - Optional engine for static evaluation
    /// * `preserve_structure` - Whether to preserve unknown object structure
    ///
    /// # Returns
    ///
    /// A compiled node, or an error if the value is invalid.
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
                    Ok(CompiledNode::StructuredObject { fields })
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
                if let Ok(opcode) = op_name.parse::<OpCode>() {
                    // Check if this operator requires array arguments
                    let requires_array = matches!(opcode, OpCode::And | OpCode::Or | OpCode::If);

                    // For operators that require arrays, check the raw value
                    if requires_array && !matches!(args_value, Value::Array(_)) {
                        // Create a special marker node for invalid arguments
                        let invalid_value = json!({
                            "__invalid_args__": true,
                            "value": args_value
                        });
                        let value_node = CompiledNode::Value {
                            value: invalid_value,
                        };
                        let args = vec![value_node];
                        return Ok(CompiledNode::BuiltinOperator { opcode, args });
                    }

                    // Special handling for preserve operator - don't compile its arguments
                    let args = if opcode == OpCode::Preserve {
                        // Preserve takes raw values, not compiled logic
                        match args_value {
                            Value::Array(arr) => arr
                                .iter()
                                .map(|v| CompiledNode::Value { value: v.clone() })
                                .collect(),
                            _ => {
                                vec![CompiledNode::Value {
                                    value: args_value.clone(),
                                }]
                            }
                        }
                    } else {
                        Self::compile_args(args_value, engine, preserve_structure)?
                    };
                    let node = CompiledNode::BuiltinOperator { opcode, args };

                    // If engine is provided and node is static, evaluate it
                    if let std::option::Option::Some(eng) = engine
                        && Self::node_is_static(&node)
                    {
                        // Evaluate with empty context since it's static
                        let mut context = ContextStack::new(Arc::new(Value::Null));
                        match eng.evaluate_node(&node, &mut context) {
                            Ok(value) => {
                                return Ok(CompiledNode::Value { value });
                            }
                            // If evaluation fails, keep as operator node
                            Err(_) => return Ok(node),
                        }
                    }

                    Ok(node)
                } else if preserve_structure {
                    // In preserve_structure mode, we need to distinguish between:
                    // 1. Custom operators (should be evaluated as operators)
                    // 2. Unknown keys (should be preserved as structured object fields)
                    //
                    // Check if this is a custom operator first
                    if let Some(eng) = engine
                        && eng.has_custom_operator(op_name)
                    {
                        // It's a registered custom operator - compile as CustomOperator
                        // This ensures custom operators work correctly in preserve_structure mode,
                        // e.g., {"result": {"custom_op": arg}} will evaluate custom_op properly
                        let args = Self::compile_args(args_value, engine, preserve_structure)?;
                        return Ok(CompiledNode::CustomOperator {
                            name: op_name.clone(),
                            args,
                        });
                    }
                    // Not a built-in operator or custom operator - treat as structured object field
                    // This allows dynamic object generation like {"name": {"var": "user.name"}}
                    let compiled_val = Self::compile_node(args_value, engine, preserve_structure)?;
                    let fields = vec![(op_name.clone(), compiled_val)];
                    Ok(CompiledNode::StructuredObject { fields })
                } else {
                    let args = Self::compile_args(args_value, engine, preserve_structure)?;
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
                    .map(|v| Self::compile_node(v, engine, preserve_structure))
                    .collect::<Result<Vec<_>>>()?;

                let nodes_boxed = nodes.into_boxed_slice();
                let node = CompiledNode::Array { nodes: nodes_boxed };

                // If engine is provided and array is static, evaluate it
                if let std::option::Option::Some(eng) = engine
                    && Self::node_is_static(&node)
                {
                    let mut context = ContextStack::new(Arc::new(Value::Null));
                    if let Ok(value) = eng.evaluate_node(&node, &mut context) {
                        return Ok(CompiledNode::Value { value });
                    }
                }

                Ok(node)
            }
            _ => {
                // Static value
                Ok(CompiledNode::Value {
                    value: value.clone(),
                })
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
                Self::opcode_is_static(opcode, args)
            }
            CompiledNode::CustomOperator { .. } => false, // Unknown operators are non-static
            CompiledNode::StructuredObject { fields, .. } => {
                fields.iter().all(|(_, node)| Self::node_is_static(node))
            }
        }
    }

    /// Check if an operator can be statically evaluated
    fn opcode_is_static(opcode: &OpCode, args: &[CompiledNode]) -> bool {
        use OpCode::*;

        // Check if all arguments are static first (common pattern)
        let args_static = || args.iter().all(Self::node_is_static);

        match opcode {
            // Context-dependent operators - always dynamic
            Var | Val | Missing | MissingSome | Exists => false,

            // Array iteration operators - always dynamic
            Map | Filter | Reduce | All | Some | None => false,

            // Error handling - dynamic
            Try | Throw => false,

            // Time-dependent - Now is always dynamic
            Now => false,

            // Special operators that need runtime info
            Preserve => false, // Operators need to know it's from an operator
            Merge | Min | Max => false, // Need to distinguish literal vs operator arrays

            // Static if arguments are static
            Type | StartsWith | EndsWith | Upper | Lower | Trim | Split | Datetime | Timestamp
            | ParseDate | FormatDate | DateDiff | Abs | Ceil | Floor | Add | Subtract
            | Multiply | Divide | Modulo | Equals | StrictEquals | NotEquals | StrictNotEquals
            | GreaterThan | GreaterThanEqual | LessThan | LessThanEqual | Not | DoubleNot | And
            | Or | Ternary | If | Cat | Substr | In | Length | Sort | Slice | Coalesce => {
                args_static()
            }
        }
    }
}
