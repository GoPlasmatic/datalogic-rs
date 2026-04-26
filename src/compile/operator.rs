//! Operator-specific compile-time specialisations.
//!
//! These convert the generic `CompiledNode::BuiltinOperator { opcode, args }`
//! form into specialised tree nodes that capture decisions at compile time:
//! - `var` / `val` → `CompiledVar` with pre-parsed segments and reduce hints.
//! - `exists` → `CompiledExists`.
//! - `split` with a named-capture regex → `CompiledSplitRegex` with the
//!   compiled `Regex` cached on the node.

use serde_json::Value;

#[cfg(feature = "ext-control")]
use crate::node::PathSegment;
use crate::node::{CompileCtx, CompiledNode, MetadataHint, ReduceHint};
#[cfg(feature = "ext-string")]
use std::sync::Arc;

#[cfg(feature = "ext-string")]
use regex::Regex;

use super::path::{parse_path_segments, parse_var_path};

/// Try to compile a `var` operator into a `CompiledVar` node.
pub(super) fn try_compile_var(args: &[CompiledNode], ctx: &mut CompileCtx) -> Option<CompiledNode> {
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
            let (hint, segs) = parse_var_path(s);
            (segs, hint)
        }
        CompiledNode::Value {
            value: Value::Number(n),
            ..
        } => {
            let s = n.to_string();
            let segs = parse_path_segments(&s);
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

/// Try to compile a `val` operator into a `CompiledVar` node.
#[cfg(feature = "ext-control")]
pub(super) fn try_compile_val(args: &[CompiledNode], ctx: &mut CompileCtx) -> Option<CompiledNode> {
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
        return try_compile_val_single_arg(&args[0], ctx);
    }

    if let CompiledNode::Value {
        value: Value::Array(level_arr),
        ..
    } = &args[0]
        && let Some(Value::Number(level_num)) = level_arr.first()
        && let Some(level) = level_num.as_i64()
    {
        let scope_level = level.unsigned_abs() as u32;
        let metadata_hint = scope_level_metadata_hint(args);
        return try_compile_val_segments(&args[1..], scope_level, metadata_hint, ctx);
    }

    if let Some(first_seg) = val_arg_to_segment(&args[0]) {
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
            try_collect_val_segments(&args[1..], &mut segments, reduce_hint, ctx)
        {
            return Some(compiled);
        }
    }

    None
}

#[cfg(feature = "ext-control")]
fn try_compile_val_single_arg(arg: &CompiledNode, ctx: &mut CompileCtx) -> Option<CompiledNode> {
    let CompiledNode::Value {
        value: Value::String(s),
        ..
    } = arg
    else {
        return None;
    };
    if s.is_empty() {
        return None;
    }
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
    Some(CompiledNode::CompiledVar {
        id: ctx.next_id(),
        scope_level: 0,
        segments: vec![segment].into_boxed_slice(),
        reduce_hint,
        metadata_hint: MetadataHint::None,
        default_value: None,
    })
}

#[cfg(feature = "ext-control")]
fn scope_level_metadata_hint(args: &[CompiledNode]) -> MetadataHint {
    if args.len() == 2
        && let CompiledNode::Value {
            value: Value::String(s),
            ..
        } = &args[1]
    {
        if s == "index" {
            return MetadataHint::Index;
        } else if s == "key" {
            return MetadataHint::Key;
        }
    }
    MetadataHint::None
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
        segments.push(val_arg_to_segment(arg)?);
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
        segments.push(val_arg_to_segment(arg)?);
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

/// Try to compile an `exists` operator into a `CompiledExists` node.
#[cfg(feature = "ext-control")]
pub(super) fn try_compile_exists(
    args: &[CompiledNode],
    ctx: &mut CompileCtx,
) -> Option<CompiledNode> {
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

/// Try to compile a `split` operator with a named-capture regex pattern
/// into a specialised `CompiledSplitRegex` node — caches the compiled
/// `Regex` on the node so it isn't re-compiled per evaluation.
#[cfg(feature = "ext-string")]
pub(super) fn try_compile_split_regex(
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
