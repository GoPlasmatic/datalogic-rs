//! `reduce` — fold an array into a single value via an accumulator.

use crate::arena::{DataContextStack, DataValue, IterGuard};
use crate::node::ReduceHint;
use crate::opcode::OpCode;
use crate::{CompiledNode, DataLogic, Result};
use bumpalo::Bump;

use super::helpers::{IterArgKind, IterSrc, ResolvedInput, resolve_iter_input};

/// `reduce` — folds an array into a single value via an accumulator. Input
/// resolves via `resolve_iter_input` (so `reduce(filter(...), +, 0)`
/// composes), with inline arithmetic fast paths for the dominant
/// `current op accumulator` pattern.
#[inline]
pub(crate) fn evaluate_reduce_arena<'a>(
    args: &'a [CompiledNode],
    iter_arg_kind: IterArgKind,
    actx: &mut DataContextStack<'a>,
    engine: &DataLogic,
    arena: &'a Bump,
) -> Result<&'a DataValue<'a>> {
    if args.len() < 2 || args.len() > 3 {
        return Err(crate::constants::invalid_args());
    }

    let body = &args[1];
    let initial: &'a DataValue<'a> = if args.len() == 3 {
        engine.evaluate_node(&args[2], actx, arena)?
    } else {
        crate::arena::pool::singleton_null()
    };

    let src = match resolve_iter_input(&args[0], iter_arg_kind, actx, engine, arena)? {
        ResolvedInput::Iterable(s) => s,
        ResolvedInput::Empty => return Ok(initial),
        ResolvedInput::Bridge(av) => {
            return reduce_arena_bridge(av, body, initial, actx, engine, arena);
        }
    };

    if src.is_empty() {
        return Ok(initial);
    }

    // FAST PATH: {op: [val("current"[+path]), val("accumulator")]} for + / - / *.
    // Skipped when a tracer is attached so per-iteration trace markers still get
    // recorded via `eval_iter_body` in the general path.
    if !actx.is_tracing()
        && let CompiledNode::BuiltinOperator {
            opcode,
            args: body_args,
            ..
        } = body
        && body_args.len() == 2
        && matches!(opcode, OpCode::Add | OpCode::Multiply | OpCode::Subtract)
        && let Some(result) = try_reduce_fast_path_arena(&src, initial, body_args, *opcode, arena)
    {
        return Ok(result);
    }

    reduce_general(&src, body, initial, actx, engine, arena)
}

/// General reduce path — push reduce frames via `IterGuard` and dispatch the
/// body per item.
#[inline]
fn reduce_general<'a>(
    src: &IterSrc<'a>,
    body: &'a CompiledNode,
    initial: &'a DataValue<'a>,
    actx: &mut DataContextStack<'a>,
    engine: &DataLogic,
    arena: &'a Bump,
) -> Result<&'a DataValue<'a>> {
    let len = src.len();
    let total = len as u32;
    let mut acc_av: &'a DataValue<'a> = initial;
    let mut guard = IterGuard::new(actx);
    for i in 0..len {
        let item = src.get(i);
        guard.step_reduce(item, acc_av);
        acc_av = engine.eval_iter_body(body, guard.stack(), arena, i as u32, total)?;
    }
    drop(guard);
    Ok(acc_av)
}

/// Reduce Bridge case — Object inputs iterate (key, value) pairs; inline
/// arena Array inputs iterate items. Non-array non-object non-null inputs
/// return the initial value.
#[inline]
fn reduce_arena_bridge<'a>(
    input: &'a DataValue<'a>,
    body: &'a CompiledNode,
    initial: &'a DataValue<'a>,
    actx: &mut DataContextStack<'a>,
    engine: &DataLogic,
    arena: &'a Bump,
) -> Result<&'a DataValue<'a>> {
    match input {
        DataValue::Object(pairs) => {
            let total = pairs.len() as u32;
            let mut acc_av: &'a DataValue<'a> = initial;
            let mut guard = IterGuard::new(actx);
            for (i, (_k, v)) in pairs.iter().enumerate() {
                // SAFETY: pairs[i].1 lives in the arena for `'a`.
                let item_av: &'a DataValue<'a> = unsafe { &*(v as *const DataValue<'a>) };
                guard.step_reduce(item_av, acc_av);
                acc_av = engine.eval_iter_body(body, guard.stack(), arena, i as u32, total)?;
            }
            drop(guard);
            Ok(acc_av)
        }
        DataValue::Array(items) => {
            let total = items.len() as u32;
            let mut acc_av: &'a DataValue<'a> = initial;
            let mut guard = IterGuard::new(actx);
            for (i, item_av) in items.iter().enumerate() {
                guard.step_reduce(item_av, acc_av);
                acc_av = engine.eval_iter_body(body, guard.stack(), arena, i as u32, total)?;
            }
            drop(guard);
            Ok(acc_av)
        }
        // Anything else — return initial.
        _ => Ok(initial),
    }
}

/// Arena variant of the reduce arithmetic fast path: detects the
/// `{+|-|*: [val("current"[+path]), val("accumulator")]}` body shape and
/// folds without per-item context push or body dispatch. Iterates `IterSrc`
/// directly.
fn try_reduce_fast_path_arena<'a>(
    src: &IterSrc<'a>,
    initial: &'a DataValue<'a>,
    body_args: &[CompiledNode],
    opcode: OpCode,
    arena: &'a Bump,
) -> Option<&'a DataValue<'a>> {
    // Identify which arg is current and which is accumulator.
    let (current_arg, _acc_arg) = match (&body_args[0], &body_args[1]) {
        (
            CompiledNode::CompiledVar {
                reduce_hint: hint0, ..
            },
            CompiledNode::CompiledVar {
                reduce_hint: hint1, ..
            },
        ) => match (hint0, hint1) {
            (
                ReduceHint::Current | ReduceHint::CurrentPath,
                ReduceHint::Accumulator | ReduceHint::AccumulatorPath,
            ) => (&body_args[0], &body_args[1]),
            (
                ReduceHint::Accumulator | ReduceHint::AccumulatorPath,
                ReduceHint::Current | ReduceHint::CurrentPath,
            ) => (&body_args[1], &body_args[0]),
            _ => return None,
        },
        _ => return None,
    };

    let current_segments = if let CompiledNode::CompiledVar {
        segments,
        reduce_hint,
        ..
    } = current_arg
    {
        match reduce_hint {
            ReduceHint::Current => &[][..],
            ReduceHint::CurrentPath => {
                if segments.len() >= 2 {
                    &segments[1..]
                } else {
                    return None;
                }
            }
            _ => return None,
        }
    } else {
        return None;
    };

    let len = src.len();

    // Integer fast path.
    let mut acc_i = initial.as_i64();
    if acc_i.is_some() {
        let mut all_int = true;
        for i in 0..len {
            let item = src.get(i);
            let current_val = if current_segments.is_empty() {
                item
            } else {
                crate::arena::value::arena_traverse_segments(item, current_segments, arena)?
            };
            if let Some(cur_i) = current_val.as_i64() {
                let a = acc_i.unwrap();
                acc_i = Some(match opcode {
                    OpCode::Add => a.wrapping_add(cur_i),
                    OpCode::Multiply => a.wrapping_mul(cur_i),
                    OpCode::Subtract => a.wrapping_sub(cur_i),
                    _ => return None,
                });
            } else {
                all_int = false;
                break;
            }
        }
        if all_int {
            return acc_i.map(|v| {
                crate::arena::pool::singleton_small_int(v).unwrap_or_else(|| {
                    &*arena.alloc(DataValue::Number(crate::value::NumberValue::from_i64(v)))
                })
            });
        }
    }

    // f64 fallback.
    let mut acc_f = initial.as_f64()?;
    for i in 0..len {
        let item = src.get(i);
        let current_val = if current_segments.is_empty() {
            item
        } else {
            crate::arena::value::arena_traverse_segments(item, current_segments, arena)?
        };
        let cur_f = current_val.as_f64()?;
        acc_f = match opcode {
            OpCode::Add => acc_f + cur_f,
            OpCode::Multiply => acc_f * cur_f,
            OpCode::Subtract => acc_f - cur_f,
            _ => return None,
        };
    }
    Some(
        arena.alloc(DataValue::Number(crate::value::NumberValue::from_f64(
            acc_f,
        ))),
    )
}
