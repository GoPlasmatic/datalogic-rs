use crate::{ContextStack, DataLogic, Result, opcode::OpCode};
use serde_json::{Value, json};
use std::sync::Arc;

/// A pre-parsed path segment for compiled variable access.
#[derive(Debug, Clone)]
pub enum PathSegment {
    /// Object field access by key
    Field(Box<str>),
    /// Array element access by index
    Index(usize),
    /// Try as object key first, then as array index (for segments that could be either).
    /// Pre-parses the index at compile time to avoid runtime parsing.
    FieldOrIndex(Box<str>, usize),
}

/// Hint for reduce context resolution, detected at compile time.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ReduceHint {
    /// Normal path access (no reduce context)
    None,
    /// Path is exactly "current" — return reduce_current directly
    Current,
    /// Path is exactly "accumulator" — return reduce_accumulator directly
    Accumulator,
    /// Path starts with "current." — segments[0] is "current", use segments[1..] from reduce_current
    CurrentPath,
    /// Path starts with "accumulator." — segments[0] is "accumulator", use segments[1..] from reduce_accumulator
    AccumulatorPath,
}

/// Hint for metadata access (index/key), detected at compile time.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MetadataHint {
    /// Normal data access
    None,
    /// Access frame index metadata
    Index,
    /// Access frame key metadata
    Key,
}

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
        args: Box<[CompiledNode]>,
    },

    /// A custom operator registered via `DataLogic::add_operator`.
    ///
    /// Custom operators use dynamic dispatch and are looked up by name
    /// from the engine's operator registry.
    CustomOperator {
        name: String,
        args: Box<[CompiledNode]>,
    },

    /// A structured object template for preserve_structure mode.
    ///
    /// When structure preservation is enabled, objects with keys that are not
    /// built-in operators or registered custom operators are preserved as templates.
    /// Each field is evaluated independently, allowing for dynamic object generation.
    ///
    /// Note: Custom operators are checked before treating keys as structured fields,
    /// ensuring they work correctly within preserved structures.
    StructuredObject {
        fields: Box<[(String, CompiledNode)]>,
    },

    /// A pre-compiled variable access (unified var/val).
    ///
    /// scope_level 0 = current context (var-style), N = go up N levels (val with [[N], ...]).
    /// Segments are pre-parsed at compile time to avoid runtime string splitting.
    CompiledVar {
        scope_level: u32,
        segments: Box<[PathSegment]>,
        reduce_hint: ReduceHint,
        metadata_hint: MetadataHint,
        default_value: Option<Box<CompiledNode>>,
    },

    /// A pre-compiled exists check.
    ///
    /// scope_level 0 = current context, N = go up N levels.
    /// Segments are pre-parsed at compile time.
    CompiledExists {
        scope_level: u32,
        segments: Box<[PathSegment]>,
    },
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

    /// Compiles for tracing without static evaluation.
    ///
    /// This method compiles the logic without performing static evaluation,
    /// ensuring that all operators remain in the tree for step-by-step debugging.
    /// Use this when you need to trace execution through operators that would
    /// otherwise be pre-evaluated at compile time.
    ///
    /// # Arguments
    ///
    /// * `logic` - The JSON logic expression to compile
    /// * `preserve_structure` - Whether to preserve unknown object structure
    ///
    /// # Returns
    ///
    /// A compiled logic structure without static optimizations.
    pub fn compile_for_trace(logic: &Value, preserve_structure: bool) -> Result<Self> {
        let root = Self::compile_node(logic, None, preserve_structure)?;
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
                    let fields: Vec<_> = obj
                        .iter()
                        .map(|(key, val)| {
                            Self::compile_node(val, engine, preserve_structure)
                                .map(|compiled_val| (key.clone(), compiled_val))
                        })
                        .collect::<Result<Vec<_>>>()?;
                    Ok(CompiledNode::StructuredObject {
                        fields: fields.into_boxed_slice(),
                    })
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
                        let args = vec![value_node].into_boxed_slice();
                        return Ok(CompiledNode::BuiltinOperator { opcode, args });
                    }

                    // Special handling for preserve operator - don't compile its arguments
                    let args = if opcode == OpCode::Preserve {
                        // Preserve takes raw values, not compiled logic
                        match args_value {
                            Value::Array(arr) => arr
                                .iter()
                                .map(|v| CompiledNode::Value { value: v.clone() })
                                .collect::<Vec<_>>()
                                .into_boxed_slice(),
                            _ => vec![CompiledNode::Value {
                                value: args_value.clone(),
                            }]
                            .into_boxed_slice(),
                        }
                    } else {
                        Self::compile_args(args_value, engine, preserve_structure)?
                    };
                    // Try to optimize variable access operators at compile time
                    if matches!(opcode, OpCode::Var | OpCode::Val | OpCode::Exists) {
                        let optimized = match opcode {
                            OpCode::Var => Self::try_compile_var(&args),
                            OpCode::Val => Self::try_compile_val(&args),
                            OpCode::Exists => Self::try_compile_exists(&args),
                            _ => None,
                        };
                        if let Some(node) = optimized {
                            return Ok(node);
                        }
                    }

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
                    let fields = vec![(op_name.clone(), compiled_val)].into_boxed_slice();
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
    ) -> Result<Box<[CompiledNode]>> {
        match value {
            Value::Array(arr) => arr
                .iter()
                .map(|v| Self::compile_node(v, engine, preserve_structure))
                .collect::<Result<Vec<_>>>()
                .map(Vec::into_boxed_slice),
            _ => {
                // Single argument - compile it
                Ok(vec![Self::compile_node(value, engine, preserve_structure)?].into_boxed_slice())
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
            CompiledNode::CompiledVar { .. } | CompiledNode::CompiledExists { .. } => false, // Context-dependent
            CompiledNode::StructuredObject { fields, .. } => {
                fields.iter().all(|(_, node)| Self::node_is_static(node))
            }
        }
    }

    /// Check if an operator can be statically evaluated at compile time.
    ///
    /// Static operators can be pre-computed during compilation when their arguments
    /// are also static, eliminating runtime evaluation overhead.
    ///
    /// # Classification Criteria
    ///
    /// An operator is **non-static** (dynamic) if it:
    /// 1. Reads from the data context (`var`, `val`, `missing`, `exists`)
    /// 2. Uses iterative callbacks with changing context (`map`, `filter`, `reduce`)
    /// 3. Has side effects or error handling (`try`, `throw`)
    /// 4. Depends on runtime state (`now` for current time)
    /// 5. Needs runtime disambiguation (`preserve`, `merge`, `min`, `max`)
    ///
    /// All other operators are **static** when their arguments are static.
    /// Parse a dot-separated path into pre-parsed segments (for var, which uses dot notation).
    /// Numeric segments become FieldOrIndex to handle both object keys and array indices.
    fn parse_path_segments(path: &str) -> Vec<PathSegment> {
        if path.is_empty() {
            return Vec::new();
        }
        if !path.contains('.') {
            if let Ok(idx) = path.parse::<usize>() {
                return vec![PathSegment::FieldOrIndex(path.into(), idx)];
            }
            return vec![PathSegment::Field(path.into())];
        }
        path.split('.')
            .map(|part| {
                if let Ok(idx) = part.parse::<usize>() {
                    PathSegment::FieldOrIndex(part.into(), idx)
                } else {
                    PathSegment::Field(part.into())
                }
            })
            .collect()
    }

    /// Parse a var path and determine the reduce hint.
    fn parse_var_path(path: &str) -> (ReduceHint, Vec<PathSegment>) {
        if path == "current" {
            (
                ReduceHint::Current,
                vec![PathSegment::Field("current".into())],
            )
        } else if path == "accumulator" {
            (
                ReduceHint::Accumulator,
                vec![PathSegment::Field("accumulator".into())],
            )
        } else if let Some(rest) = path.strip_prefix("current.") {
            let mut segs = vec![PathSegment::Field("current".into())];
            segs.extend(Self::parse_path_segments(rest));
            (ReduceHint::CurrentPath, segs)
        } else if let Some(rest) = path.strip_prefix("accumulator.") {
            let mut segs = vec![PathSegment::Field("accumulator".into())];
            segs.extend(Self::parse_path_segments(rest));
            (ReduceHint::AccumulatorPath, segs)
        } else {
            (ReduceHint::None, Self::parse_path_segments(path))
        }
    }

    /// Try to compile a var operator into a CompiledVar node.
    fn try_compile_var(args: &[CompiledNode]) -> Option<CompiledNode> {
        if args.is_empty() {
            return Some(CompiledNode::CompiledVar {
                scope_level: 0,
                segments: Box::new([]),
                reduce_hint: ReduceHint::None,
                metadata_hint: MetadataHint::None,
                default_value: None,
            });
        }

        let (segments, reduce_hint) = match &args[0] {
            CompiledNode::Value {
                value: Value::String(s),
            } => {
                let (hint, segs) = Self::parse_var_path(s);
                (segs, hint)
            }
            CompiledNode::Value {
                value: Value::Number(n),
            } => {
                let s = n.to_string();
                let segs = Self::parse_path_segments(&s);
                (segs, ReduceHint::None)
            }
            _ => return None, // dynamic path
        };

        let default_value = if args.len() > 1 {
            Some(Box::new(args[1].clone()))
        } else {
            None
        };

        Some(CompiledNode::CompiledVar {
            scope_level: 0,
            segments: segments.into_boxed_slice(),
            reduce_hint,
            metadata_hint: MetadataHint::None,
            default_value,
        })
    }

    /// Try to compile a val operator into a CompiledVar node.
    fn try_compile_val(args: &[CompiledNode]) -> Option<CompiledNode> {
        if args.is_empty() {
            return Some(CompiledNode::CompiledVar {
                scope_level: 0,
                segments: Box::new([]),
                reduce_hint: ReduceHint::None,
                metadata_hint: MetadataHint::None,
                default_value: None,
            });
        }

        // Val does NOT support dot-path notation. Each arg is a literal key/index.

        // Case 2: Single non-empty string → single Field segment (literal key)
        // Empty string has dual behavior (try key "" then whole-context fallback) — keep as BuiltinOperator.
        if args.len() == 1 {
            if let CompiledNode::Value {
                value: Value::String(s),
            } = &args[0]
                && !s.is_empty()
            {
                let reduce_hint = if s == "current" {
                    ReduceHint::Current
                } else if s == "accumulator" {
                    ReduceHint::Accumulator
                } else {
                    ReduceHint::None
                };
                let segment = if let Ok(idx) = s.parse::<usize>() {
                    PathSegment::FieldOrIndex(s.as_str().into(), idx)
                } else {
                    PathSegment::Field(s.as_str().into())
                };
                return Some(CompiledNode::CompiledVar {
                    scope_level: 0,
                    segments: vec![segment].into_boxed_slice(),
                    reduce_hint,
                    metadata_hint: MetadataHint::None,
                    default_value: None,
                });
            }
            return None;
        }

        // Case 3: First arg is [[level]] array
        if let CompiledNode::Value {
            value: Value::Array(level_arr),
        } = &args[0]
            && let Some(Value::Number(level_num)) = level_arr.first()
            && let Some(level) = level_num.as_u64()
        {
            // Check metadata hints for 2-arg case
            let mut metadata_hint = MetadataHint::None;
            if args.len() == 2
                && let CompiledNode::Value {
                    value: Value::String(s),
                } = &args[1]
            {
                if s == "index" {
                    metadata_hint = MetadataHint::Index;
                } else if s == "key" {
                    metadata_hint = MetadataHint::Key;
                }
            }

            return Self::try_compile_val_segments(&args[1..], level as u32, metadata_hint);
        }

        // Case 4: 2+ args with all literal path segments — compile as path chain.
        if let Some(first_seg) = Self::val_arg_to_segment(&args[0]) {
            let reduce_hint = match &args[0] {
                CompiledNode::Value {
                    value: Value::String(s),
                } if s == "current" => ReduceHint::CurrentPath,
                CompiledNode::Value {
                    value: Value::String(s),
                } if s == "accumulator" => ReduceHint::AccumulatorPath,
                _ => ReduceHint::None,
            };

            let mut segments = vec![first_seg];
            if let Some(compiled) =
                Self::try_collect_val_segments(&args[1..], &mut segments, reduce_hint)
            {
                return Some(compiled);
            }
        }

        None
    }

    /// Convert a val argument into a PathSegment.
    /// Val treats string args as literal keys (no dot-splitting), and numbers as indices.
    /// Numeric strings get FieldOrIndex to handle both object key and array index access.
    fn val_arg_to_segment(arg: &CompiledNode) -> Option<PathSegment> {
        match arg {
            CompiledNode::Value {
                value: Value::String(s),
            } => {
                if let Ok(idx) = s.parse::<usize>() {
                    Some(PathSegment::FieldOrIndex(s.as_str().into(), idx))
                } else {
                    Some(PathSegment::Field(s.as_str().into()))
                }
            }
            CompiledNode::Value {
                value: Value::Number(n),
            } => n.as_u64().map(|idx| PathSegment::Index(idx as usize)),
            _ => None,
        }
    }

    /// Try to compile val path segments (used by level-access and path-chain cases).
    fn try_compile_val_segments(
        args: &[CompiledNode],
        scope_level: u32,
        metadata_hint: MetadataHint,
    ) -> Option<CompiledNode> {
        let mut segments = Vec::new();
        for arg in args {
            segments.push(Self::val_arg_to_segment(arg)?);
        }

        Some(CompiledNode::CompiledVar {
            scope_level,
            segments: segments.into_boxed_slice(),
            reduce_hint: ReduceHint::None,
            metadata_hint,
            default_value: None,
        })
    }

    /// Try to collect remaining val args into segments and build a CompiledVar.
    fn try_collect_val_segments(
        args: &[CompiledNode],
        segments: &mut Vec<PathSegment>,
        reduce_hint: ReduceHint,
    ) -> Option<CompiledNode> {
        for arg in args {
            segments.push(Self::val_arg_to_segment(arg)?);
        }

        Some(CompiledNode::CompiledVar {
            scope_level: 0,
            segments: std::mem::take(segments).into_boxed_slice(),
            reduce_hint,
            metadata_hint: MetadataHint::None,
            default_value: None,
        })
    }

    /// Try to compile an exists operator into a CompiledExists node.
    fn try_compile_exists(args: &[CompiledNode]) -> Option<CompiledNode> {
        if args.is_empty() {
            return Some(CompiledNode::CompiledExists {
                scope_level: 0,
                segments: Box::new([]),
            });
        }

        if args.len() == 1 {
            if let CompiledNode::Value {
                value: Value::String(s),
            } = &args[0]
            {
                return Some(CompiledNode::CompiledExists {
                    scope_level: 0,
                    segments: vec![PathSegment::Field(s.as_str().into())].into_boxed_slice(),
                });
            }
            return None;
        }

        // Multiple args - all must be literal strings
        let mut segments = Vec::new();
        for arg in args {
            if let CompiledNode::Value {
                value: Value::String(s),
            } = arg
            {
                segments.push(PathSegment::Field(s.as_str().into()));
            } else {
                return None;
            }
        }

        Some(CompiledNode::CompiledExists {
            scope_level: 0,
            segments: segments.into_boxed_slice(),
        })
    }

    fn opcode_is_static(opcode: &OpCode, args: &[CompiledNode]) -> bool {
        use OpCode::*;

        // Check if all arguments are static first (common pattern)
        let args_static = || args.iter().all(Self::node_is_static);

        match opcode {
            // Context-dependent: These operators read from the data context, which is
            // not available at compile time. They must remain dynamic.
            Var | Val | Missing | MissingSome | Exists => false,

            // Iteration operators: These push new contexts for each iteration and use
            // callbacks that may reference the iteration variable. Even with static
            // arrays, the callback logic depends on the per-element context.
            Map | Filter | Reduce | All | Some | None => false,

            // Error handling: These have control flow effects (early exit, error propagation)
            // that should be preserved for runtime execution.
            Try | Throw => false,

            // Time-dependent: Returns current UTC time, inherently non-static.
            Now => false,

            // Runtime disambiguation needed:
            // - Preserve: Must know it was explicitly used as an operator, not inferred
            // - Merge/Min/Max: Need to distinguish [1,2,3] literal from operator arguments
            //   at runtime to handle nested arrays correctly
            Preserve => false,
            Merge | Min | Max => false,

            // Pure operators: Static when all arguments are static. These perform
            // deterministic transformations without side effects or context access.
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
