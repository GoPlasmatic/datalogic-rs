//! `reduce` — fold an array into a single value via an accumulator.

use crate::arena::{ContextStack, DataValue, IterGuard};
use crate::node::{PathSegment, ReduceHint};
use crate::opcode::OpCode;
use crate::{CompiledNode, Engine, Result};
use bumpalo::Bump;
use datavalue::NumberValue;

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
/// The fused loop. Both the per-item map and the fold run through
/// [`arith_number`], which applies the binary arithmetic operators' own
/// representation rules — so the fused result matches what the unfused
/// pipeline computes through general dispatch by construction, rather
/// than by a hand-maintained mirror of it.
///
/// That equivalence is load-bearing and was previously wrong. The old
/// implementation carried the accumulator as a raw `f64` across a
/// three-mode state machine (int/int, int-map + f64-fold, full f64). The
/// unfused pipeline instead rebuilds a `NumberValue` every step, and
/// `NumberValue::from_f64` collapses a whole, exactly-i64-representable
/// result back to `Integer` — which flips the *next* step from f64 math
/// into exact i64 math. Above 2^53 those disagree: folding
/// `[-9591485970090907; 6]` with `{"-": [current, accumulator]}` from
/// `0.25` gave 0 fused and 1 unfused (issue #61).
///
/// Anything non-numeric still bails to the general flow, which owns
/// coercion.
fn run_fused_fold<'a>(
    src: &IterSrc<'a>,
    initial: &'a DataValue<'a>,
    fold: &FoldShape<'_>,
    map_body: &FusedMapBody<'_>,
    arena: &'a Bump,
) -> FusedOutcome<'a> {
    let op = fold.op;
    let acc_is_lhs = fold.acc_is_lhs;

    // Pre-coerce an ArithVarLit literal once. Non-numeric literal: the map
    // fast path would decline too — bail to the general flow's coercion.
    let lit = match map_body {
        FusedMapBody::ArithVarLit { lit, .. } => match lit.as_number() {
            Some(n) => *n,
            None => return FusedOutcome::Bail,
        },
        _ => NumberValue::from_i64(0),
    };

    let mut cursors = FusedCursors::new(map_body);

    let Some(mut acc) = initial.as_number().copied() else {
        return FusedOutcome::Bail;
    };
    for i in 0..src.len() {
        let item = src.get(i);
        let Some(mapped) = mapped_number(map_body, &mut cursors, item, lit) else {
            return FusedOutcome::Bail;
        };
        let Some(next) = fold_number(op, acc_is_lhs, acc, mapped) else {
            return FusedOutcome::Bail;
        };
        acc = next;
    }
    FusedOutcome::Done(alloc_number(arena, acc))
}

/// Allocate a fold result, short-circuiting to the preallocated small-int
/// singletons the way both fast paths did before.
#[inline(always)]
fn alloc_number<'a>(arena: &'a Bump, n: NumberValue) -> &'a DataValue<'a> {
    if let NumberValue::Integer(i) = n {
        if let Some(singleton) = crate::arena::singletons::singleton_small_int(i) {
            return singleton;
        }
    }
    arena.alloc(DataValue::Number(n))
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

/// One item's mapped value. `None` on a missing field or a non-numeric
/// value — the caller bails to the general flow, which owns coercion.
#[inline(always)]
fn mapped_number<'a>(
    map_body: &FusedMapBody<'_>,
    cursors: &mut FusedCursors<'_>,
    item: &'a DataValue<'a>,
    lit: NumberValue,
) -> Option<NumberValue> {
    match map_body {
        FusedMapBody::Extract { .. } => cursors.a.resolve(item)?.as_number().copied(),
        FusedMapBody::ArithVarLit { op, var_is_lhs, .. } => {
            let v = *cursors.a.resolve(item)?.as_number()?;
            let (x, y) = if *var_is_lhs { (v, lit) } else { (lit, v) };
            arith_number(*op, x, y)
        }
        FusedMapBody::ArithVarVar { op, .. } => {
            let a = *cursors.a.resolve(item)?.as_number()?;
            let b = *cursors.b.as_mut()?.resolve(item)?.as_number()?;
            arith_number(*op, a, b)
        }
    }
}

/// Checked integer combine for one of `+` / `-` / `*`; `None` signals
/// overflow, which promotes to the float form.
type IntOp = fn(i64, i64) -> Option<i64>;
/// The matching float combine, used on overflow or a non-integral operand.
type FloatOp = fn(f64, f64) -> f64;

/// `a op b` with the binary arithmetic operators' exact representation
/// rules: integer math whenever both operands land exactly in `i64`
/// (which includes whole floats), promoting through
/// [`try_int_op`](crate::operators::arithmetic::try_int_op) on overflow,
/// and `from_f64` otherwise. `None` for an op outside the detected
/// `+` / `-` / `*` set.
///
/// Deliberately *not* `NumberValue::add`/`sub`/`mul`: those return a bare
/// `Float` on integer overflow, where the operators return
/// `from_f64(..)`, which collapses a whole in-range result back to
/// `Integer`. `reduce([1], {"-": [accumulator, current]}, i64::MIN)` is
/// the case that separates them.
#[inline(always)]
fn arith_number(op: OpCode, a: NumberValue, b: NumberValue) -> Option<NumberValue> {
    let (int_op, float_op): (IntOp, FloatOp) = match op {
        OpCode::Add => (i64::checked_add, |x, y| x + y),
        OpCode::Subtract => (i64::checked_sub, |x, y| x - y),
        OpCode::Multiply => (i64::checked_mul, |x, y| x * y),
        _ => return None,
    };
    Some(match (a.as_i64(), b.as_i64()) {
        (Some(x), Some(y)) => crate::operators::arithmetic::try_int_op(x, y, int_op, float_op),
        _ => NumberValue::from_f64(float_op(a.as_f64(), b.as_f64())),
    })
}

/// One fold step, honouring which operand the accumulator sits on so
/// non-commutative folds keep their order.
#[inline(always)]
fn fold_number(
    op: OpCode,
    acc_is_lhs: bool,
    acc: NumberValue,
    cur: NumberValue,
) -> Option<NumberValue> {
    if acc_is_lhs {
        arith_number(op, acc, cur)
    } else {
        arith_number(op, cur, acc)
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
///
/// Folds through [`arith_number`] for the same reason [`run_fused_fold`]
/// does: it applies the arithmetic operators' representation rules, so the
/// int/float decision at every step is identical to general dispatch
/// instead of mirrored. The previous two-pass form (exact i64 while it
/// fit, then a full restart in raw f64) diverged above 2^53, and did so
/// without a map in play — issue #61 reproduced through this path too.
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
    // Hinted per-row resolver for `current.path` folds.
    let mut current_field = FieldCursor::new(current_segments);

    let mut acc = initial.as_number().copied()?;
    for i in 0..src.len() {
        let item = src.get(i);
        let cur = *current_field.resolve(item)?.as_number()?;
        acc = fold_number(op, acc_is_lhs, acc, cur)?;
    }
    Some(alloc_number(arena, acc))
}
