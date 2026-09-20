//! Operator-specific compile-time specialisations.
//!
//! These convert the generic `CompiledNode::BuiltinOperator { opcode, args }`
//! form into specialised tree nodes that capture decisions at compile time:
//! - `var` / `val` → `CompiledVar` with pre-parsed segments and reduce hints.
//! - `exists` → `CompiledExists`.

use crate::node::PathSegment;
use crate::node::{CompileCtx, CompiledNode, MetadataHint, ReduceHint, ScopeBinding};

use super::path_segments::{parse_path_segments, parse_var_path, str_to_segment};

/// Build the empty-args `var` / `val` node: root scope, no segments, no hints.
fn empty_var(ctx: &mut CompileCtx) -> CompiledNode {
    CompiledNode::Var {
        id: Some(ctx.next_id()),
        scope_level: 0,
        segments: Box::new([]),
        reduce_hint: ReduceHint::None,
        metadata_hint: MetadataHint::None,
        default_value: None,
        binding: ScopeBinding::Unresolved,
    }
}

/// Try to compile a `var` operator into a `CompiledVar` node.
pub(super) fn try_compile_var(args: &[CompiledNode], ctx: &mut CompileCtx) -> Option<CompiledNode> {
    if args.is_empty() {
        return Some(empty_var(ctx));
    }

    // A leading level marker makes this the `val` form, not `[path, default]`:
    // `var` compiles to `val` and the runtime already reads it that way, so
    // hand it over rather than keeping a second copy of the marker branch.
    if literal_level_marker(&args[0]).is_some() {
        return try_compile_val(args, ctx);
    }

    let (segments, reduce_hint) = match &args[0] {
        CompiledNode::Value {
            value: datavalue::OwnedDataValue::String(s),
            ..
        } => {
            let (hint, segs) = parse_var_path(s);
            (segs, hint)
        }
        CompiledNode::Value {
            value: datavalue::OwnedDataValue::Number(n),
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

    Some(CompiledNode::Var {
        id: Some(ctx.next_id()),
        scope_level: 0,
        segments: segments.into_boxed_slice(),
        reduce_hint,
        metadata_hint: MetadataHint::None,
        default_value,
        binding: ScopeBinding::Unresolved,
    })
}

/// Try to compile a `val` operator into a `CompiledVar` node.
pub(super) fn try_compile_val(args: &[CompiledNode], ctx: &mut CompileCtx) -> Option<CompiledNode> {
    if args.is_empty() {
        return Some(empty_var(ctx));
    }

    // A level marker covers the bare `{"val": [[N]]}` too — the tail is empty
    // and a metadata hint needs a second argument — so it is tested first.
    if let Some(scope_level) = literal_level_marker(&args[0]) {
        let metadata_hint = scope_level_metadata_hint(args, scope_level);
        return finish_val(
            &args[1..],
            Vec::new(),
            scope_level,
            ReduceHint::None,
            metadata_hint,
            ctx,
        );
    }

    if args.len() == 1 {
        return try_compile_val_single_arg(&args[0], ctx);
    }

    if let Some(first_seg) = val_arg_to_segment(&args[0]) {
        let reduce_hint = match &args[0] {
            CompiledNode::Value {
                value: datavalue::OwnedDataValue::String(s),
                ..
            } if s == "current" => ReduceHint::CurrentPath,
            CompiledNode::Value {
                value: datavalue::OwnedDataValue::String(s),
                ..
            } if s == "accumulator" => ReduceHint::AccumulatorPath,
            _ => ReduceHint::None,
        };

        let segments = vec![first_seg];
        if let Some(compiled) = finish_val(
            &args[1..],
            segments,
            0,
            reduce_hint,
            MetadataHint::None,
            ctx,
        ) {
            return Some(compiled);
        }
    }

    None
}

fn try_compile_val_single_arg(arg: &CompiledNode, ctx: &mut CompileCtx) -> Option<CompiledNode> {
    let CompiledNode::Value {
        value: datavalue::OwnedDataValue::String(s),
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
    let segment = str_to_segment(s);
    Some(CompiledNode::Var {
        id: Some(ctx.next_id()),
        scope_level: 0,
        segments: vec![segment].into_boxed_slice(),
        reduce_hint,
        metadata_hint: MetadataHint::None,
        default_value: None,
        binding: ScopeBinding::Unresolved,
    })
}

/// The level a literal `[N]` marker names, magnitude only, saturating rather
/// than truncating so a level past `u32::MAX` cannot wrap to 0 and silently
/// read the current frame.
fn literal_level_marker(arg: &CompiledNode) -> Option<u32> {
    let CompiledNode::Value {
        value: datavalue::OwnedDataValue::Array(level_arr),
        ..
    } = arg
    else {
        return None;
    };
    let Some(datavalue::OwnedDataValue::Number(level_num)) = level_arr.first() else {
        return None;
    };
    let level = level_num.as_i64()?;
    Some(level.unsigned_abs().min(u32::MAX as u64) as u32)
}

/// `index` / `key` read iteration metadata only where the level names a
/// metadata frame. At an even, non-zero level they are ordinary field names,
/// so the hint stays `None` and the segment does the work.
fn scope_level_metadata_hint(args: &[CompiledNode], scope_level: u32) -> MetadataHint {
    if crate::arena::metadata_climb(scope_level as usize).is_none() {
        return MetadataHint::None;
    }
    if args.len() == 2
        && let CompiledNode::Value {
            value: datavalue::OwnedDataValue::String(s),
            ..
        } = &args[1]
    {
        return MetadataHint::from_path(s);
    }
    MetadataHint::None
}

fn val_arg_to_segment(arg: &CompiledNode) -> Option<PathSegment> {
    match arg {
        CompiledNode::Value {
            value: datavalue::OwnedDataValue::String(s),
            ..
        } => Some(str_to_segment(s)),
        CompiledNode::Value {
            value: datavalue::OwnedDataValue::Number(n),
            ..
        } => n
            .as_i64()
            .filter(|i| *i >= 0)
            .map(|i| PathSegment::Index(i as usize)),
        _ => None,
    }
}

/// Append each remaining `val` arg as a path segment onto `segments` and
/// finish a `Var` node. Shared by the `[level]`-prefixed form (seed empty,
/// non-zero scope, metadata hint) and the leading-segment form (seed with the
/// first segment, reduce hint). Returns `None` if any arg is not a segment.
fn finish_val(
    args: &[CompiledNode],
    mut segments: Vec<PathSegment>,
    scope_level: u32,
    reduce_hint: ReduceHint,
    metadata_hint: MetadataHint,
    ctx: &mut CompileCtx,
) -> Option<CompiledNode> {
    for arg in args {
        segments.push(val_arg_to_segment(arg)?);
    }

    Some(CompiledNode::Var {
        id: Some(ctx.next_id()),
        scope_level,
        segments: segments.into_boxed_slice(),
        reduce_hint,
        metadata_hint,
        default_value: None,
        binding: ScopeBinding::Unresolved,
    })
}

/// Try to compile an `exists` operator into a `CompiledExists` node.
#[cfg(feature = "ext-control")]
pub(super) fn try_compile_exists(
    args: &[CompiledNode],
    ctx: &mut CompileCtx,
) -> Option<CompiledNode> {
    if args.is_empty() {
        return Some(CompiledNode::Exists(Box::new(
            crate::node::CompiledExistsData {
                id: Some(ctx.next_id()),
                scope_level: 0,
                segments: Box::new([]),
                binding: ScopeBinding::Unresolved,
            },
        )));
    }

    if args.len() == 1 {
        if let CompiledNode::Value {
            value: datavalue::OwnedDataValue::String(s),
            ..
        } = &args[0]
        {
            return Some(CompiledNode::Exists(Box::new(
                crate::node::CompiledExistsData {
                    id: Some(ctx.next_id()),
                    scope_level: 0,
                    segments: vec![PathSegment::Field(s.as_str().into())].into_boxed_slice(),
                    binding: ScopeBinding::Unresolved,
                },
            )));
        }
        return None;
    }

    let mut segments = Vec::new();
    for arg in args {
        if let CompiledNode::Value {
            value: datavalue::OwnedDataValue::String(s),
            ..
        } = arg
        {
            segments.push(PathSegment::Field(s.as_str().into()));
        } else {
            return None;
        }
    }

    Some(CompiledNode::Exists(Box::new(
        crate::node::CompiledExistsData {
            id: Some(ctx.next_id()),
            scope_level: 0,
            segments: segments.into_boxed_slice(),
            binding: ScopeBinding::Unresolved,
        },
    )))
}
