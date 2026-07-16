//! `reduce` — fold an array into a single value via an accumulator.

use crate::arena::{ContextStack, DataValue, IterGuard};
use crate::node::{PathSegment, ReduceHint};
use crate::opcode::OpCode;
use crate::{CompiledNode, Engine, Result};
use bumpalo::Bump;

use super::helpers::{
    FieldCursor, FusedMapBody, IterArgKind, IterSrc, ResolvedInput, resolve_iter_input,
};

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

    // FUSION: reduce over a map with a fusible body folds directly over the
    // map's input — no intermediate array materializes. Runs after `initial`
    // evaluates (order preserved) and before `args[0]` resolves; on Bail the
    // general flow below re-resolves `args[0]`, re-evaluating the pure map
    // input (the established fast-path precedent — fires only on
    // non-numeric data). The inline candidate pre-check keeps non-pipeline
    // reduces at two discriminant compares.
    if !ctx.is_tracing() && is_map_candidate(&args[0]) {
        match try_fused_reduce_map(args, initial, ctx, engine, arena)? {
            FusedOutcome::Done(value) => return Ok(value),
            FusedOutcome::Bail => {}
        }
    }

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

/// Outcome of the reduce(map(...)) fusion attempt.
enum FusedOutcome<'a> {
    /// The fused loop completed; this is the reduce result.
    Done(&'a DataValue<'a>),
    /// Shape or data didn't fit — fall through to the general flow.
    Bail,
}

/// Cheap inline pre-gate for the fusion attempt: is `args[0]` a `map`
/// node (possibly behind a CSE wrapper)?
#[inline(always)]
fn is_map_candidate(node: &CompiledNode) -> bool {
    let node = match node {
        CompiledNode::Cse(data) => &data.inner,
        node => node,
    };
    matches!(
        node,
        CompiledNode::BuiltinOperator {
            opcode: OpCode::Map,
            ..
        }
    )
}

/// Fuse `reduce({map: [input, <fusible body>]}, <two-var fold>, initial)`
/// into a single pass over `input`, never materializing the intermediate
/// array. Detection is purely structural (the shared [`FusedMapBody`] plus
/// the fold shape); the loops compose only existing primitives (`as_i64`,
/// `as_f64`, checked ops, `NumberValue::from_f64`) so results are
/// bit-identical to the unfused pipeline, and anything non-numeric bails
/// to the untouched general flow.
#[inline(never)]
fn try_fused_reduce_map<'a>(
    args: &'a [CompiledNode],
    initial: &'a DataValue<'a>,
    ctx: &mut ContextStack<'a>,
    engine: &Engine,
    arena: &'a Bump,
) -> Result<FusedOutcome<'a>> {
    // See through a CSE wrapper: a memoized pipeline computes its memo miss
    // right here, and the wrapped inner map then never materializes (its
    // own slot simply stays lazy for any standalone occurrence).
    let map_node = match &args[0] {
        CompiledNode::Cse(data) => &data.inner,
        node => node,
    };
    let CompiledNode::BuiltinOperator {
        opcode: OpCode::Map,
        args: map_args,
        iter_arg_kind: map_iter_kind,
        ..
    } = map_node
    else {
        return Ok(FusedOutcome::Bail);
    };
    if map_args.len() != 2 {
        return Ok(FusedOutcome::Bail);
    }
    let Some(fold) = detect_fold_shape(&args[1]) else {
        return Ok(FusedOutcome::Bail);
    };
    // The fold must read bare `current` — a path under `current` would
    // index into the mapped element, which the fused loop never builds.
    if !fold.current_segments.is_empty() {
        return Ok(FusedOutcome::Bail);
    }
    let Some(map_body) = FusedMapBody::detect(&map_args[1]) else {
        return Ok(FusedOutcome::Bail);
    };

    let src = match resolve_iter_input(&map_args[0], *map_iter_kind, ctx, engine, arena)? {
        ResolvedInput::Iterable(s) => s,
        ResolvedInput::Empty => return Ok(FusedOutcome::Done(initial)),
        ResolvedInput::Bridge(_) => return Ok(FusedOutcome::Bail),
    };
    if src.is_empty() {
        return Ok(FusedOutcome::Done(initial));
    }
    Ok(run_fused_fold(&src, initial, &fold, &map_body, arena))
}

/// Representation to restart with after the integer mode aborts.
enum FusedRestart {
    /// Fold overflowed (or the initial value is fractional) while every
    /// mapped value so far was integral: keep exact integer map results and
    /// fold them in f64 — exactly what the unfused pipeline does when it
    /// f64-folds a materialized `Integer` array.
    IntMapF64Fold,
    /// A mapped value wasn't integral: the unfused map would have produced
    /// the whole intermediate array in f64, so recompute everything in f64.
    FullF64,
}

/// The fused loops. Three modes mirror the unfused pipeline's
/// representation choices so outputs stay bit-identical:
/// integer map + integer fold; integer map + f64 fold (after fold overflow
/// or a fractional initial); whole-array f64 map + f64 fold (once any
/// mapped value is non-integral). Anything non-numeric bails.
fn run_fused_fold<'a>(
    src: &IterSrc<'a>,
    initial: &'a DataValue<'a>,
    fold: &FoldShape<'_>,
    map_body: &FusedMapBody<'_>,
    arena: &'a Bump,
) -> FusedOutcome<'a> {
    let len = src.len();
    let op = fold.op;
    let acc_is_lhs = fold.acc_is_lhs;

    // Pre-coerce an ArithVarLit literal once. Non-numeric literal: the map
    // fast path would decline too — bail to the general flow's coercion.
    let (lit_i, lit_f) = match map_body {
        FusedMapBody::ArithVarLit { lit, .. } => {
            let Some(f) = lit.as_f64() else {
                return FusedOutcome::Bail;
            };
            (lit.as_i64(), f)
        }
        _ => (Some(0), 0.0),
    };

    let mut cursors = FusedCursors::new(map_body);

    // Integer mode.
    let restart = 'int_mode: {
        // A fractional literal makes integer map math impossible — the
        // unfused map would produce the whole array in f64.
        let Some(lit_i) = lit_i else {
            break 'int_mode FusedRestart::FullF64;
        };
        let Some(mut acc) = initial.as_i64() else {
            break 'int_mode FusedRestart::IntMapF64Fold;
        };
        for i in 0..len {
            let item = src.get(i);
            let Some(mapped) = mapped_i64(map_body, &mut cursors, item, lit_i) else {
                break 'int_mode FusedRestart::FullF64;
            };
            let Some(next) = fold_i64(op, acc_is_lhs, acc, mapped) else {
                break 'int_mode FusedRestart::IntMapF64Fold;
            };
            acc = next;
        }
        return FusedOutcome::Done(
            crate::arena::singletons::singleton_small_int(acc).unwrap_or_else(|| {
                &*arena.alloc(DataValue::Number(datavalue::NumberValue::from_i64(acc)))
            }),
        );
    };

    let Some(init_f) = initial.as_f64() else {
        return FusedOutcome::Bail;
    };

    // Integer-map / f64-fold mode.
    if matches!(restart, FusedRestart::IntMapF64Fold) {
        if let Some(lit_i) = lit_i {
            let mut acc_f = init_f;
            let mut all_int = true;
            for i in 0..len {
                let item = src.get(i);
                let Some(mapped) = mapped_i64(map_body, &mut cursors, item, lit_i) else {
                    all_int = false;
                    break;
                };
                acc_f = fold_f64(op, acc_is_lhs, acc_f, mapped as f64);
            }
            if all_int {
                return FusedOutcome::Done(
                    arena.alloc(DataValue::Number(datavalue::NumberValue::from_f64(acc_f))),
                );
            }
            // A non-integral value past the overflow point: the unfused map
            // would have produced the whole array in f64 — fall through.
        }
    }

    // Full f64 mode.
    let mut acc_f = init_f;
    for i in 0..len {
        let item = src.get(i);
        let Some(mapped) = mapped_f64(map_body, &mut cursors, item, lit_f) else {
            return FusedOutcome::Bail;
        };
        acc_f = fold_f64(op, acc_is_lhs, acc_f, mapped);
    }
    FusedOutcome::Done(arena.alloc(DataValue::Number(datavalue::NumberValue::from_f64(acc_f))))
}

/// Field cursors for the map body's var operands; persist across mode
/// restarts so the hinted lookups stay warm.
struct FusedCursors<'n> {
    a: FieldCursor<'n>,
    b: Option<FieldCursor<'n>>,
}

impl<'n> FusedCursors<'n> {
    fn new(map_body: &FusedMapBody<'n>) -> Self {
        match map_body {
            FusedMapBody::Extract { segments } => Self {
                a: FieldCursor::new(segments),
                b: None,
            },
            FusedMapBody::ArithVarLit { segments, .. } => Self {
                a: FieldCursor::new(segments),
                b: None,
            },
            FusedMapBody::ArithVarVar {
                a_segments,
                b_segments,
                ..
            } => Self {
                a: FieldCursor::new(a_segments),
                b: Some(FieldCursor::new(b_segments)),
            },
        }
    }
}

/// One item's mapped value in integer math. `None` on a missing field,
/// non-integral value, or map-op overflow.
#[inline(always)]
fn mapped_i64<'a>(
    map_body: &FusedMapBody<'_>,
    cursors: &mut FusedCursors<'_>,
    item: &'a DataValue<'a>,
    lit_i: i64,
) -> Option<i64> {
    match map_body {
        FusedMapBody::Extract { .. } => cursors.a.resolve(item)?.as_i64(),
        FusedMapBody::ArithVarLit { op, var_is_lhs, .. } => {
            let v = cursors.a.resolve(item)?.as_i64()?;
            let (x, y) = if *var_is_lhs { (v, lit_i) } else { (lit_i, v) };
            arith_i64(*op, x, y)
        }
        FusedMapBody::ArithVarVar { op, .. } => {
            let a = cursors.a.resolve(item)?.as_i64()?;
            let b = cursors.b.as_mut()?.resolve(item)?.as_i64()?;
            arith_i64(*op, a, b)
        }
    }
}

/// One item's mapped value in f64 math. `None` on a missing field or a
/// non-numeric value — the caller bails to the general flow.
#[inline(always)]
fn mapped_f64<'a>(
    map_body: &FusedMapBody<'_>,
    cursors: &mut FusedCursors<'_>,
    item: &'a DataValue<'a>,
    lit_f: f64,
) -> Option<f64> {
    match map_body {
        FusedMapBody::Extract { .. } => cursors.a.resolve(item)?.as_f64(),
        FusedMapBody::ArithVarLit { op, var_is_lhs, .. } => {
            let v = cursors.a.resolve(item)?.as_f64()?;
            let (x, y) = if *var_is_lhs { (v, lit_f) } else { (lit_f, v) };
            Some(arith_f64(*op, x, y))
        }
        FusedMapBody::ArithVarVar { op, .. } => {
            let a = cursors.a.resolve(item)?.as_f64()?;
            let b = cursors.b.as_mut()?.resolve(item)?.as_f64()?;
            Some(arith_f64(*op, a, b))
        }
    }
}

#[inline(always)]
fn arith_i64(op: OpCode, a: i64, b: i64) -> Option<i64> {
    match op {
        OpCode::Add => a.checked_add(b),
        OpCode::Subtract => a.checked_sub(b),
        OpCode::Multiply => a.checked_mul(b),
        _ => None,
    }
}

#[inline(always)]
fn arith_f64(op: OpCode, a: f64, b: f64) -> f64 {
    match op {
        OpCode::Add => a + b,
        OpCode::Subtract => a - b,
        OpCode::Multiply => a * b,
        // Detection admits only + / - / *.
        _ => unreachable!("fused map body ops are Add/Subtract/Multiply"),
    }
}

#[inline(always)]
fn fold_i64(op: OpCode, acc_is_lhs: bool, acc: i64, cur: i64) -> Option<i64> {
    match op {
        OpCode::Add => acc.checked_add(cur),
        OpCode::Multiply => acc.checked_mul(cur),
        OpCode::Subtract if acc_is_lhs => acc.checked_sub(cur),
        OpCode::Subtract => cur.checked_sub(acc),
        _ => None,
    }
}

#[inline(always)]
fn fold_f64(op: OpCode, acc_is_lhs: bool, acc: f64, cur: f64) -> f64 {
    match op {
        OpCode::Add => acc + cur,
        OpCode::Multiply => acc * cur,
        OpCode::Subtract if acc_is_lhs => acc - cur,
        OpCode::Subtract => cur - acc,
        // `detect_fold_shape` admits only + / - / *.
        _ => unreachable!("fold ops are Add/Subtract/Multiply"),
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
