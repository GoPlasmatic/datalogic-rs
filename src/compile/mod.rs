pub mod optimize;

use crate::node::{
    CompileCtx, CompiledLogic, CompiledNode, MetadataHint, PathSegment, ReduceHint, SYNTHETIC_ID,
    node_is_static,
};
use crate::opcode::OpCode;
use crate::{DataLogic, Result};
#[cfg(feature = "ext-string")]
use regex::Regex;
use serde_json::{Value, json};
use std::sync::Arc;

impl CompiledLogic {
    /// Compiles a JSON value into a compiled logic structure.
    ///
    /// This method performs basic compilation without static evaluation.
    /// For optimal performance, use `compile_with_static_eval` instead.
    pub fn compile(logic: &Value) -> Result<Self> {
        let mut ctx = CompileCtx::new();
        let root = Self::compile_node(logic, None, false, &mut ctx)?;
        Ok(Self::new(root))
    }

    /// Compiles for tracing without static evaluation.
    ///
    /// This method compiles the logic without performing static evaluation,
    /// ensuring that all operators remain in the tree for step-by-step debugging.
    #[cfg(feature = "trace")]
    pub fn compile_for_trace(logic: &Value, preserve_structure: bool) -> Result<Self> {
        let mut ctx = CompileCtx::new();
        let root = Self::compile_node(logic, None, preserve_structure, &mut ctx)?;
        Ok(Self::new(root))
    }

    /// Compiles with static evaluation using the provided engine.
    pub fn compile_with_static_eval(logic: &Value, engine: &DataLogic) -> Result<Self> {
        let mut ctx = CompileCtx::new();
        let root = Self::compile_node(logic, Some(engine), engine.preserve_structure(), &mut ctx)?;
        Ok(Self::new(root))
    }

    /// Compiles a single JSON value into a CompiledNode.
    fn compile_node(
        value: &Value,
        engine: Option<&DataLogic>,
        preserve_structure: bool,
        ctx: &mut CompileCtx,
    ) -> Result<CompiledNode> {
        match value {
            Value::Object(obj) if obj.len() > 1 => {
                #[cfg(feature = "preserve")]
                if preserve_structure {
                    // In preserve_structure mode, treat multi-key objects as structured objects
                    let fields: Vec<_> = obj
                        .iter()
                        .map(|(key, val)| {
                            Self::compile_node(val, engine, preserve_structure, ctx)
                                .map(|compiled_val| (key.clone(), compiled_val))
                        })
                        .collect::<Result<Vec<_>>>()?;
                    return Ok(CompiledNode::StructuredObject(Box::new(
                        crate::node::StructuredObjectData {
                            id: ctx.next_id(),
                            fields: fields.into_boxed_slice(),
                        },
                    )));
                }
                {
                    let _ = obj;
                    Err(crate::error::Error::InvalidOperator(
                        "Unknown Operator".to_string(),
                    ))
                }
            }
            Value::Object(obj) if obj.len() == 1 => {
                let (op_name, args_value) = obj.iter().next().unwrap();

                if let Ok(opcode) = op_name.parse::<OpCode>() {
                    let requires_array = matches!(opcode, OpCode::And | OpCode::Or | OpCode::If);

                    if requires_array && !matches!(args_value, Value::Array(_)) {
                        // Create a special marker node for invalid arguments
                        let invalid_value = json!({
                            "__invalid_args__": true,
                            "value": args_value
                        });
                        let value_node = CompiledNode::value_with_id(ctx.next_id(), invalid_value);
                        let args = vec![value_node].into_boxed_slice();
                        return Ok(CompiledNode::BuiltinOperator {
                            id: ctx.next_id(),
                            opcode,
                            args,
                        });
                    }

                    #[cfg(feature = "preserve")]
                    let args = if opcode == OpCode::Preserve {
                        // Preserve takes raw values, not compiled logic
                        match args_value {
                            Value::Array(arr) => arr
                                .iter()
                                .map(|v| CompiledNode::value_with_id(ctx.next_id(), v.clone()))
                                .collect::<Vec<_>>()
                                .into_boxed_slice(),
                            _ => vec![CompiledNode::value_with_id(
                                ctx.next_id(),
                                args_value.clone(),
                            )]
                            .into_boxed_slice(),
                        }
                    } else {
                        Self::compile_args(args_value, engine, preserve_structure, ctx)?
                    };
                    #[cfg(not(feature = "preserve"))]
                    let args = Self::compile_args(args_value, engine, preserve_structure, ctx)?;

                    if opcode == OpCode::Var
                        && let Some(node) = Self::try_compile_var(&args, ctx)
                    {
                        return Ok(node);
                    }
                    #[cfg(feature = "ext-control")]
                    if matches!(opcode, OpCode::Val | OpCode::Exists) {
                        let optimized = match opcode {
                            OpCode::Val => Self::try_compile_val(&args, ctx),
                            OpCode::Exists => Self::try_compile_exists(&args, ctx),
                            _ => None,
                        };
                        if let Some(node) = optimized {
                            return Ok(node);
                        }
                    }

                    #[cfg(feature = "ext-string")]
                    if opcode == OpCode::Split
                        && let Some(node) = Self::try_compile_split_regex(&args, ctx)
                    {
                        return Ok(node);
                    }

                    #[cfg(feature = "error-handling")]
                    if opcode == OpCode::Throw
                        && args.len() == 1
                        && let CompiledNode::Value {
                            value: Value::String(s),
                            ..
                        } = &args[0]
                    {
                        return Ok(CompiledNode::CompiledThrow(Box::new(
                            crate::node::CompiledThrowData {
                                id: ctx.next_id(),
                                error: serde_json::json!({"type": s}),
                            },
                        )));
                    }

                    let mut node = CompiledNode::BuiltinOperator {
                        id: ctx.next_id(),
                        opcode,
                        args,
                    };

                    // Run optimization passes when engine is available
                    if let std::option::Option::Some(eng) = engine {
                        node = optimize::optimize(node, eng);
                    }

                    // If engine is provided and node is static, evaluate it
                    if let std::option::Option::Some(eng) = engine
                        && node_is_static(&node)
                    {
                        match optimize::constant_fold::fold_static_node(&node, eng) {
                            Some(value) => {
                                return Ok(CompiledNode::value_with_id(ctx.next_id(), value));
                            }
                            None => return Ok(node),
                        }
                    }

                    return Ok(node);
                }

                #[cfg(feature = "preserve")]
                if preserve_structure {
                    if let Some(eng) = engine
                        && eng.has_custom_operator(op_name)
                    {
                        let args = Self::compile_args(args_value, engine, preserve_structure, ctx)?;
                        return Ok(CompiledNode::CustomOperator(Box::new(
                            crate::node::CustomOperatorData {
                                id: ctx.next_id(),
                                name: op_name.clone(),
                                args,
                            },
                        )));
                    }
                    let compiled_val =
                        Self::compile_node(args_value, engine, preserve_structure, ctx)?;
                    let fields = vec![(op_name.clone(), compiled_val)].into_boxed_slice();
                    return Ok(CompiledNode::StructuredObject(Box::new(
                        crate::node::StructuredObjectData {
                            id: ctx.next_id(),
                            fields,
                        },
                    )));
                }

                {
                    let args = Self::compile_args(args_value, engine, preserve_structure, ctx)?;
                    Ok(CompiledNode::CustomOperator(Box::new(
                        crate::node::CustomOperatorData {
                            id: ctx.next_id(),
                            name: op_name.clone(),
                            args,
                        },
                    )))
                }
            }
            Value::Array(arr) => {
                let nodes = arr
                    .iter()
                    .map(|v| Self::compile_node(v, engine, preserve_structure, ctx))
                    .collect::<Result<Vec<_>>>()?;

                let nodes_boxed = nodes.into_boxed_slice();
                let node = CompiledNode::Array {
                    id: ctx.next_id(),
                    nodes: nodes_boxed,
                };

                if let std::option::Option::Some(eng) = engine
                    && node_is_static(&node)
                    && let Some(value) = optimize::constant_fold::fold_static_node(&node, eng)
                {
                    return Ok(CompiledNode::value_with_id(ctx.next_id(), value));
                }

                Ok(node)
            }
            _ => Ok(CompiledNode::value_with_id(ctx.next_id(), value.clone())),
        }
    }

    /// Compile operator arguments
    fn compile_args(
        value: &Value,
        engine: Option<&DataLogic>,
        preserve_structure: bool,
        ctx: &mut CompileCtx,
    ) -> Result<Box<[CompiledNode]>> {
        match value {
            Value::Array(arr) => arr
                .iter()
                .map(|v| Self::compile_node(v, engine, preserve_structure, ctx))
                .collect::<Result<Vec<_>>>()
                .map(Vec::into_boxed_slice),
            _ => Ok(
                vec![Self::compile_node(value, engine, preserve_structure, ctx)?]
                    .into_boxed_slice(),
            ),
        }
    }

    /// Parse a dot-separated path into pre-parsed segments.
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
    fn try_compile_var(args: &[CompiledNode], ctx: &mut CompileCtx) -> Option<CompiledNode> {
        if args.is_empty() {
            return Some(CompiledNode::CompiledVar {
                id: ctx.next_id(),
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
                ..
            } => {
                let (hint, segs) = Self::parse_var_path(s);
                (segs, hint)
            }
            CompiledNode::Value {
                value: Value::Number(n),
                ..
            } => {
                let s = n.to_string();
                let segs = Self::parse_path_segments(&s);
                (segs, ReduceHint::None)
            }
            _ => return None,
        };

        let default_value = if args.len() > 1 {
            Some(Box::new(args[1].clone()))
        } else {
            None
        };

        Some(CompiledNode::CompiledVar {
            id: ctx.next_id(),
            scope_level: 0,
            segments: segments.into_boxed_slice(),
            reduce_hint,
            metadata_hint: MetadataHint::None,
            default_value,
        })
    }

    /// Try to compile a val operator into a CompiledVar node.
    #[cfg(feature = "ext-control")]
    fn try_compile_val(args: &[CompiledNode], ctx: &mut CompileCtx) -> Option<CompiledNode> {
        if args.is_empty() {
            return Some(CompiledNode::CompiledVar {
                id: ctx.next_id(),
                scope_level: 0,
                segments: Box::new([]),
                reduce_hint: ReduceHint::None,
                metadata_hint: MetadataHint::None,
                default_value: None,
            });
        }

        if args.len() == 1 {
            if let CompiledNode::Value {
                value: Value::String(s),
                ..
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
                    id: ctx.next_id(),
                    scope_level: 0,
                    segments: vec![segment].into_boxed_slice(),
                    reduce_hint,
                    metadata_hint: MetadataHint::None,
                    default_value: None,
                });
            }
            return None;
        }

        if let CompiledNode::Value {
            value: Value::Array(level_arr),
            ..
        } = &args[0]
            && let Some(Value::Number(level_num)) = level_arr.first()
            && let Some(level) = level_num.as_i64()
        {
            let scope_level = level.unsigned_abs() as u32;

            let mut metadata_hint = MetadataHint::None;
            if args.len() == 2
                && let CompiledNode::Value {
                    value: Value::String(s),
                    ..
                } = &args[1]
            {
                if s == "index" {
                    metadata_hint = MetadataHint::Index;
                } else if s == "key" {
                    metadata_hint = MetadataHint::Key;
                }
            }

            return Self::try_compile_val_segments(&args[1..], scope_level, metadata_hint, ctx);
        }

        if let Some(first_seg) = Self::val_arg_to_segment(&args[0]) {
            let reduce_hint = match &args[0] {
                CompiledNode::Value {
                    value: Value::String(s),
                    ..
                } if s == "current" => ReduceHint::CurrentPath,
                CompiledNode::Value {
                    value: Value::String(s),
                    ..
                } if s == "accumulator" => ReduceHint::AccumulatorPath,
                _ => ReduceHint::None,
            };

            let mut segments = vec![first_seg];
            if let Some(compiled) =
                Self::try_collect_val_segments(&args[1..], &mut segments, reduce_hint, ctx)
            {
                return Some(compiled);
            }
        }

        None
    }

    #[cfg(feature = "ext-control")]
    fn val_arg_to_segment(arg: &CompiledNode) -> Option<PathSegment> {
        match arg {
            CompiledNode::Value {
                value: Value::String(s),
                ..
            } => {
                if let Ok(idx) = s.parse::<usize>() {
                    Some(PathSegment::FieldOrIndex(s.as_str().into(), idx))
                } else {
                    Some(PathSegment::Field(s.as_str().into()))
                }
            }
            CompiledNode::Value {
                value: Value::Number(n),
                ..
            } => n.as_u64().map(|idx| PathSegment::Index(idx as usize)),
            _ => None,
        }
    }

    #[cfg(feature = "ext-control")]
    fn try_compile_val_segments(
        args: &[CompiledNode],
        scope_level: u32,
        metadata_hint: MetadataHint,
        ctx: &mut CompileCtx,
    ) -> Option<CompiledNode> {
        let mut segments = Vec::new();
        for arg in args {
            segments.push(Self::val_arg_to_segment(arg)?);
        }

        Some(CompiledNode::CompiledVar {
            id: ctx.next_id(),
            scope_level,
            segments: segments.into_boxed_slice(),
            reduce_hint: ReduceHint::None,
            metadata_hint,
            default_value: None,
        })
    }

    #[cfg(feature = "ext-control")]
    fn try_collect_val_segments(
        args: &[CompiledNode],
        segments: &mut Vec<PathSegment>,
        reduce_hint: ReduceHint,
        ctx: &mut CompileCtx,
    ) -> Option<CompiledNode> {
        for arg in args {
            segments.push(Self::val_arg_to_segment(arg)?);
        }

        Some(CompiledNode::CompiledVar {
            id: ctx.next_id(),
            scope_level: 0,
            segments: std::mem::take(segments).into_boxed_slice(),
            reduce_hint,
            metadata_hint: MetadataHint::None,
            default_value: None,
        })
    }

    #[cfg(feature = "ext-control")]
    fn try_compile_exists(args: &[CompiledNode], ctx: &mut CompileCtx) -> Option<CompiledNode> {
        if args.is_empty() {
            return Some(CompiledNode::CompiledExists(Box::new(
                crate::node::CompiledExistsData {
                    id: ctx.next_id(),
                    scope_level: 0,
                    segments: Box::new([]),
                },
            )));
        }

        if args.len() == 1 {
            if let CompiledNode::Value {
                value: Value::String(s),
                ..
            } = &args[0]
            {
                return Some(CompiledNode::CompiledExists(Box::new(
                    crate::node::CompiledExistsData {
                        id: ctx.next_id(),
                        scope_level: 0,
                        segments: vec![PathSegment::Field(s.as_str().into())].into_boxed_slice(),
                    },
                )));
            }
            return None;
        }

        let mut segments = Vec::new();
        for arg in args {
            if let CompiledNode::Value {
                value: Value::String(s),
                ..
            } = arg
            {
                segments.push(PathSegment::Field(s.as_str().into()));
            } else {
                return None;
            }
        }

        Some(CompiledNode::CompiledExists(Box::new(
            crate::node::CompiledExistsData {
                id: ctx.next_id(),
                scope_level: 0,
                segments: segments.into_boxed_slice(),
            },
        )))
    }

    #[cfg(feature = "ext-string")]
    fn try_compile_split_regex(
        args: &[CompiledNode],
        ctx: &mut CompileCtx,
    ) -> Option<CompiledNode> {
        if args.len() < 2 {
            return None;
        }

        let pattern = match &args[1] {
            CompiledNode::Value {
                value: Value::String(s),
                ..
            } if s.contains("(?P<") => s.as_str(),
            _ => return None,
        };

        let re = Regex::new(pattern).ok()?;
        let capture_names: Vec<Box<str>> = re.capture_names().flatten().map(|n| n.into()).collect();

        if capture_names.is_empty() {
            return None;
        }

        let text_args = vec![args[0].clone()].into_boxed_slice();

        Some(CompiledNode::CompiledSplitRegex(Box::new(
            crate::node::CompiledSplitRegexData {
                id: ctx.next_id(),
                args: text_args,
                regex: Arc::new(re),
                capture_names: capture_names.into_boxed_slice(),
            },
        )))
    }
}

// Re-export SYNTHETIC_ID so `Self::...` usages in this file don't break if a
// future refactor wants to construct synthetic nodes here.
#[allow(dead_code)]
pub(crate) const _SYNTHETIC_REXP: u32 = SYNTHETIC_ID;
