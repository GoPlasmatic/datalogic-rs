pub mod optimize;

use crate::node::{
    CompiledLogic, CompiledNode, MetadataHint, PathSegment, ReduceHint, node_is_static,
};
use crate::opcode::OpCode;
use crate::{ContextStack, DataLogic, Result};
#[cfg(feature = "ext-string")]
use regex::Regex;
use serde_json::{Value, json};
use std::sync::Arc;

impl CompiledLogic {
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
    #[cfg(feature = "trace")]
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
                #[cfg(feature = "preserve")]
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
                    return Ok(CompiledNode::StructuredObject(Box::new(
                        crate::node::StructuredObjectData {
                            fields: fields.into_boxed_slice(),
                        },
                    )));
                }
                {
                    // Multi-key objects are not valid operators
                    let _ = obj;
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
                    #[cfg(feature = "preserve")]
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
                    #[cfg(not(feature = "preserve"))]
                    let args = Self::compile_args(args_value, engine, preserve_structure)?;
                    // Try to optimize variable access operators at compile time
                    if opcode == OpCode::Var
                        && let Some(node) = Self::try_compile_var(&args)
                    {
                        return Ok(node);
                    }
                    #[cfg(feature = "ext-control")]
                    if matches!(opcode, OpCode::Val | OpCode::Exists) {
                        let optimized = match opcode {
                            OpCode::Val => Self::try_compile_val(&args),
                            OpCode::Exists => Self::try_compile_exists(&args),
                            _ => None,
                        };
                        if let Some(node) = optimized {
                            return Ok(node);
                        }
                    }

                    // Pre-compile regex for split operator when delimiter is a static pattern
                    #[cfg(feature = "ext-string")]
                    if opcode == OpCode::Split
                        && let Some(node) = Self::try_compile_split_regex(&args)
                    {
                        return Ok(node);
                    }

                    // Pre-compile throw with literal string into CompiledThrow
                    #[cfg(feature = "error-handling")]
                    if opcode == OpCode::Throw
                        && args.len() == 1
                        && let CompiledNode::Value {
                            value: Value::String(s),
                        } = &args[0]
                    {
                        return Ok(CompiledNode::CompiledThrow(Box::new(
                            serde_json::json!({"type": s}),
                        )));
                    }

                    let mut node = CompiledNode::BuiltinOperator { opcode, args };

                    // Run optimization passes when engine is available
                    if let std::option::Option::Some(eng) = engine {
                        node = optimize::optimize(node, eng);
                    }

                    // If engine is provided and node is static, evaluate it
                    if let std::option::Option::Some(eng) = engine
                        && node_is_static(&node)
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

                    return Ok(node);
                }

                #[cfg(feature = "preserve")]
                if preserve_structure {
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
                        return Ok(CompiledNode::CustomOperator(Box::new(
                            crate::node::CustomOperatorData {
                                name: op_name.clone(),
                                args,
                            },
                        )));
                    }
                    // Not a built-in operator or custom operator - treat as structured object field
                    // This allows dynamic object generation like {"name": {"var": "user.name"}}
                    let compiled_val = Self::compile_node(args_value, engine, preserve_structure)?;
                    let fields = vec![(op_name.clone(), compiled_val)].into_boxed_slice();
                    return Ok(CompiledNode::StructuredObject(Box::new(
                        crate::node::StructuredObjectData { fields },
                    )));
                }

                {
                    let args = Self::compile_args(args_value, engine, preserve_structure)?;
                    // Fall back to custom operator - don't pre-evaluate custom operators
                    Ok(CompiledNode::CustomOperator(Box::new(
                        crate::node::CustomOperatorData {
                            name: op_name.clone(),
                            args,
                        },
                    )))
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
                    && node_is_static(&node)
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
    #[cfg(feature = "ext-control")]
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
            && let Some(level) = level_num.as_i64()
        {
            let scope_level = level.unsigned_abs() as u32;

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

            return Self::try_compile_val_segments(&args[1..], scope_level, metadata_hint);
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
    #[cfg(feature = "ext-control")]
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
    #[cfg(feature = "ext-control")]
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
    #[cfg(feature = "ext-control")]
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
    #[cfg(feature = "ext-control")]
    fn try_compile_exists(args: &[CompiledNode]) -> Option<CompiledNode> {
        if args.is_empty() {
            return Some(CompiledNode::CompiledExists(Box::new(
                crate::node::CompiledExistsData {
                    scope_level: 0,
                    segments: Box::new([]),
                },
            )));
        }

        if args.len() == 1 {
            if let CompiledNode::Value {
                value: Value::String(s),
            } = &args[0]
            {
                return Some(CompiledNode::CompiledExists(Box::new(
                    crate::node::CompiledExistsData {
                        scope_level: 0,
                        segments: vec![PathSegment::Field(s.as_str().into())].into_boxed_slice(),
                    },
                )));
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

        Some(CompiledNode::CompiledExists(Box::new(
            crate::node::CompiledExistsData {
                scope_level: 0,
                segments: segments.into_boxed_slice(),
            },
        )))
    }

    /// Try to pre-compile a split operator's regex pattern at compile time.
    ///
    /// When the delimiter (second arg) is a static string containing named capture
    /// groups (`(?P<...>`), the regex is compiled once here instead of on every evaluation.
    #[cfg(feature = "ext-string")]
    fn try_compile_split_regex(args: &[CompiledNode]) -> Option<CompiledNode> {
        if args.len() < 2 {
            return None;
        }

        // Check if the delimiter is a static string with named capture groups
        let pattern = match &args[1] {
            CompiledNode::Value {
                value: Value::String(s),
            } if s.contains("(?P<") => s.as_str(),
            _ => return None,
        };

        // Try to compile the regex
        let re = Regex::new(pattern).ok()?;
        let capture_names: Vec<Box<str>> = re.capture_names().flatten().map(|n| n.into()).collect();

        // Only optimize if there are named capture groups
        if capture_names.is_empty() {
            return None;
        }

        // Keep only the text argument (first arg)
        let text_args = vec![args[0].clone()].into_boxed_slice();

        Some(CompiledNode::CompiledSplitRegex(Box::new(
            crate::node::CompiledSplitRegexData {
                args: text_args,
                regex: Arc::new(re),
                capture_names: capture_names.into_boxed_slice(),
            },
        )))
    }
}
