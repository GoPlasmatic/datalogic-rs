//! `reduce` — fold an array into a single value via an accumulator.

use crate::arena::{ContextStack, DataValue, IterGuard};
use crate::node::{PathSegment, ReduceHint};
use crate::opcode::OpCode;
use crate::{CompiledNode, Engine, Result};
use bumpalo::Bump;

use super::helpers::{IterArgKind, IterSrc, ResolvedInput, resolve_iter_input};

/// `reduce` — folds an array into a single value via an accumulator. Input
/// resolves via `resolve_iter_input` (so `reduce(filter(...), +, 0)`
/// composes), with an inline arithmetic fast path for two-var `+`/`-`/`*`
/// fold bodies in either operand order.
#[inline]
pub(crate) fn evaluate_reduce<'a>(
    args: &'a [CompiledNode],
    iter_arg_kind: IterArgKind,
    ctx: &mut ContextStack<'a>,
    engine: &Engine,
    arena: &'a Bump,
) -> Result<&'a DataValue<'a>> {
    if args.len() < 2 || args.len() > 3 {
        return Err(crate::Error::invalid_args());
    }

    let body = &args[1];
    let initial: &'a DataValue<'a> = if args.len() == 3 {
        engine.dispatch_node(&args[2], ctx, arena)?
    } else {
        crate::arena::singletons::singleton_null()
    };

    let src = match resolve_iter_input(&args[0], iter_arg_kind, ctx, engine, arena)? {
        ResolvedInput::Iterable(s) => s,
        ResolvedInput::Empty => return Ok(initial),
        ResolvedInput::Bridge(av) => {
            return reduce_arena_bridge(av, body, initial, ctx, engine, arena);
        }
    };

    if src.is_empty() {
        return Ok(initial);
    }

    // FAST PATH: {op: [val("current"[+path]), val("accumulator")]} in either
    // operand order for + / - / *. Skipped when a tracer is attached so
    // per-iteration trace markers still get recorded via `run_iter_body` in
    // the general path.
    if !ctx.is_tracing() {
        if let Some(result) = try_reduce_fast_path(&src, initial, body, arena) {
            return Ok(result);
        }
    }

    reduce_general(&src, body, initial, ctx, engine, arena)
}

/// General reduce path — push reduce frames via `IterGuard` and dispatch the
/// body per item.
#[inline]
fn reduce_general<'a>(
    src: &IterSrc<'a>,
    body: &'a CompiledNode,
    initial: &'a DataValue<'a>,
    ctx: &mut ContextStack<'a>,
    engine: &Engine,
    arena: &'a Bump,
) -> Result<&'a DataValue<'a>> {
    let len = src.len();
    let total = len as u32;
    let mut acc_av: &'a DataValue<'a> = initial;
    let mut guard = IterGuard::new(ctx);
    for i in 0..len {
        let item = src.get(i);
        guard.step_reduce(item, acc_av);
        acc_av = engine.run_iter_body(body, guard.stack(), arena, i as u32, total)?;
    }
    drop(guard);
    Ok(acc_av)
}

/// Reduce Bridge case — Object inputs iterate (key, value) pairs. The Bridge
/// variant is only produced for non-null, non-array values (`value_as_iter`
/// routes Null to Empty and Array to Iterable), so every other shape returns
/// the initial value.
#[inline]
fn reduce_arena_bridge<'a>(
    input: &'a DataValue<'a>,
    body: &'a CompiledNode,
    initial: &'a DataValue<'a>,
    ctx: &mut ContextStack<'a>,
    engine: &Engine,
    arena: &'a Bump,
) -> Result<&'a DataValue<'a>> {
    match input {
        DataValue::Object(pairs) => {
            let total = pairs.len() as u32;
            let mut acc_av: &'a DataValue<'a> = initial;
            let mut guard = IterGuard::new(ctx);
            for (i, (_k, v)) in pairs.iter().enumerate() {
                guard.step_reduce(v, acc_av);
                acc_av = engine.run_iter_body(body, guard.stack(), arena, i as u32, total)?;
            }
            drop(guard);
            Ok(acc_av)
        }
        // Anything else (scalars, strings) — return initial. Null and Array
        // never reach the Bridge variant, so they are not handled here.
        _ => Ok(initial),
    }
}

/// Detected `{+|-|*: [var, var]}` fold body over `current`/`accumulator`,
/// operand order preserved.
struct FoldShape<'a> {
    op: OpCode,
    /// true — body is `{op: [accumulator, current]}`; false — `[current, accumulator]`.
    acc_is_lhs: bool,
    /// Path below `current` (`"current.x.y"` → `["x", "y"]`); empty for bare `current`.
    current_segments: &'a [PathSegment],
}

/// Matches a reduce body of the shape `{+|-|*: [val("current"[+path]),
/// val("accumulator")]}` in either operand order, recording which side the
/// accumulator sits on so non-commutative folds evaluate correctly.
fn detect_fold_shape(body: &CompiledNode) -> Option<FoldShape<'_>> {
    let (opcode, body_args) = match body {
        CompiledNode::BuiltinOperator { opcode, args, .. } => (*opcode, args),
        _ => return None,
    };
    if body_args.len() != 2 || !matches!(opcode, OpCode::Add | OpCode::Multiply | OpCode::Subtract)
    {
        return None;
    }

    // Identify which arg is current and which is accumulator.
    let (current_arg, acc_is_lhs) = match (&body_args[0], &body_args[1]) {
        (
            CompiledNode::Var {
                reduce_hint: hint0, ..
            },
            CompiledNode::Var {
                reduce_hint: hint1, ..
            },
        ) => match (hint0, hint1) {
            (
                ReduceHint::Current | ReduceHint::CurrentPath,
                ReduceHint::Accumulator | ReduceHint::AccumulatorPath,
            ) => (&body_args[0], false),
            (
                ReduceHint::Accumulator | ReduceHint::AccumulatorPath,
                ReduceHint::Current | ReduceHint::CurrentPath,
            ) => (&body_args[1], true),
            _ => return None,
        },
        _ => return None,
    };

    let current_segments = if let CompiledNode::Var {
        segments,
        reduce_hint,
        ..
    } = current_arg
    {
        match reduce_hint {
            ReduceHint::Current => &[][..],
            ReduceHint::CurrentPath if segments.len() >= 2 => &segments[1..],
            _ => return None,
        }
    } else {
        return None;
    };

    Some(FoldShape {
        op: opcode,
        acc_is_lhs,
        current_segments,
    })
}

/// Arena variant of the reduce arithmetic fast path: detects a `FoldShape`
/// body and folds without per-item context push or body dispatch. Iterates
/// `IterSrc` directly.
fn try_reduce_fast_path<'a>(
    src: &IterSrc<'a>,
    initial: &'a DataValue<'a>,
    body: &CompiledNode,
    arena: &'a Bump,
) -> Option<&'a DataValue<'a>> {
    let FoldShape {
        op,
        acc_is_lhs,
        current_segments,
    } = detect_fold_shape(body)?;
    let len = src.len();

    // Integer fast path. `acc` stays a plain i64 for the duration of the
    // block (it is only ever reassigned an integer), so no Option juggling.
    if let Some(mut acc) = initial.as_i64() {
        let mut all_int = true;
        for i in 0..len {
            let item = src.get(i);
            let current_val = if current_segments.is_empty() {
                item
            } else {
                crate::arena::value::traverse_segments(item, current_segments)?
            };
            if let Some(cur_i) = current_val.as_i64() {
                let checked = match op {
                    OpCode::Add => acc.checked_add(cur_i),
                    OpCode::Multiply => acc.checked_mul(cur_i),
                    OpCode::Subtract if acc_is_lhs => acc.checked_sub(cur_i),
                    OpCode::Subtract => cur_i.checked_sub(acc),
                    _ => return None,
                };
                match checked {
                    Some(next) => acc = next,
                    // On i64 overflow, abandon the integer path and let the
                    // f64 fallback recompute from `initial`. This matches the
                    // general arithmetic path, which promotes to f64 rather
                    // than wrapping.
                    None => {
                        all_int = false;
                        break;
                    }
                }
            } else {
                all_int = false;
                break;
            }
        }
        if all_int {
            return Some(
                crate::arena::singletons::singleton_small_int(acc).unwrap_or_else(|| {
                    &*arena.alloc(DataValue::Number(datavalue::NumberValue::from_i64(acc)))
                }),
            );
        }
    }

    // f64 fallback.
    let mut acc_f = initial.as_f64()?;
    for i in 0..len {
        let item = src.get(i);
        let current_val = if current_segments.is_empty() {
            item
        } else {
            crate::arena::value::traverse_segments(item, current_segments)?
        };
        let cur_f = current_val.as_f64()?;
        acc_f = match op {
            OpCode::Add => acc_f + cur_f,
            OpCode::Multiply => acc_f * cur_f,
            OpCode::Subtract if acc_is_lhs => acc_f - cur_f,
            OpCode::Subtract => cur_f - acc_f,
            _ => return None,
        };
    }
    Some(arena.alloc(DataValue::Number(datavalue::NumberValue::from_f64(acc_f))))
}
