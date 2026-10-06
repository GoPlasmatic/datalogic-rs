//! `reduce` — fold an array into a single value via an accumulator.

use crate::OpCode;
use crate::arena::{ContextStack, DataValue, IterGuard};
use crate::node::{PathSegment, ReduceHint};
use crate::operators::meta::ArithOp;
use crate::{CompiledNode, Engine, Result};
use bumpalo::Bump;
use datavalue::NumberValue;

use super::fused::{FieldCursor, FusedMapBody, arith_number, with_arith};
use super::input::{Items, IterArgKind, IterSrc, ResolvedInput, resolve_iter_input, resolve_value};
use super::nesting::AccumulatorDepth;

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
    let body = &args[1];
    let initial: &'a DataValue<'a> = if args.len() == 3 {
        engine.dispatch_node(&args[2], ctx, arena)?
    } else {
        crate::arena::singletons::singleton_null()
    };

    // FUSION: reduce over a map with a fusible body folds directly over the
    // map's input — no intermediate array materializes. Runs after `initial`
    // evaluates (order preserved) and before `args[0]` resolves. Once the
    // fusion has resolved the map's input it never hands back a bare Bail:
    // data the fused loop cannot fold is mapped by the general `map` over
    // that same input (`Mapped`), so nothing is evaluated or charged
    // twice. The inline candidate pre-check keeps non-pipeline reduces at
    // two discriminant compares.
    let resolved = 'source: {
        if super::fast_paths::allowed(ctx, engine) && is_map_candidate(&args[0]) {
            match try_fused_reduce_map(args, initial, ctx, engine, arena)? {
                FusedOutcome::Done(value) => return Ok(value),
                FusedOutcome::Mapped(mapped) => break 'source resolve_value(mapped, ctx)?,
                FusedOutcome::Bail => {}
            }
        }
        resolve_iter_input(&args[0], iter_arg_kind, ctx, engine, arena)?
    };

    let src = match resolved {
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
    if super::fast_paths::allowed(ctx, engine)
        && let Some(result) = try_reduce_fast_path(&src, initial, body, arena)
    {
        // The fold body's three nodes per item, as the general path
        // charges them.
        ctx.charge(FOLD_BODY_COST * src.len() as u64)?;
        return Ok(result);
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
    let mut depth = AccumulatorDepth::new(body);
    let mut guard = IterGuard::new(ctx);
    for i in 0..len {
        let item = src.get(i);
        guard.step_reduce(item, acc_av);
        acc_av = engine.run_iter_body(body, guard.stack(), arena, i as u32, total)?;
        depth.step(acc_av)?;
    }
    drop(guard);
    depth.finish(acc_av)?;
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
            let mut depth = AccumulatorDepth::new(body);
            let mut guard = IterGuard::new(ctx);
            for (i, (_k, v)) in pairs.iter().enumerate() {
                guard.step_reduce(v, acc_av);
                acc_av = engine.run_iter_body(body, guard.stack(), arena, i as u32, total)?;
                depth.step(acc_av)?;
            }
            drop(guard);
            depth.finish(acc_av)?;
            Ok(acc_av)
        }
        // Anything else (scalars, strings) — return initial. Null and Array
        // never reach the Bridge variant, so they are not handled here.
        _ => Ok(initial),
    }
}

/// Operations the general path charges per item for a fold body the fast
/// paths recognise: the arithmetic node and its two `var`s.
const FOLD_BODY_COST: u64 = 3;

/// Outcome of the reduce(map(...)) fusion attempt.
enum FusedOutcome<'a> {
    /// The fused loop completed; this is the reduce result.
    Done(&'a DataValue<'a>),
    /// The map's input was resolved but the data did not fit the fused
    /// loop, so the general `map` ran over it: this is the map's result,
    /// for the reduce to continue from.
    Mapped(&'a DataValue<'a>),
    /// The shape didn't fit; nothing was evaluated. Fall through to the
    /// general flow.
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
/// bit-identical to the unfused pipeline. Anything non-numeric goes to the
/// general `map` over the already-resolved input (see [`FusedOutcome`]).
///
/// The metered count is the unfused pipeline's: the map node, its input,
/// and per item the map body, the mapped item the reduce examines and the
/// fold body.
#[inline(never)]
fn try_fused_reduce_map<'a>(
    args: &'a [CompiledNode],
    initial: &'a DataValue<'a>,
    ctx: &mut ContextStack<'a>,
    engine: &Engine,
    arena: &'a Bump,
) -> Result<FusedOutcome<'a>> {
    // See through a CSE wrapper: a memoized pipeline computes its memo miss
    // right here. A fused fold never builds the mapped array, so it leaves
    // the slot empty for any standalone occurrence; the general `map`
    // below fills it, as dispatching the wrapper would.
    let (map_node, memo_slot) = match &args[0] {
        CompiledNode::Cse(data) => (&data.inner, Some(data.slot)),
        node => (node, None),
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
    // What the fused loop cannot fold whatever the data: a non-numeric
    // initial value or map literal. Known before anything is evaluated.
    let Some(acc) = initial.as_number().copied() else {
        return Ok(FusedOutcome::Bail);
    };
    let lit = match &map_body {
        FusedMapBody::ArithVarLit { lit, .. } => match lit.as_number() {
            Some(n) => *n,
            None => return Ok(FusedOutcome::Bail),
        },
        _ => NumberValue::from_i64(0),
    };
    // A memoised map already computed costs the general flow one read.
    if let Some(slot) = memo_slot
        && ctx.cse_slot(slot).is_some()
    {
        return Ok(FusedOutcome::Bail);
    }

    let fused = FusedMap {
        map_args,
        map_iter_kind: *map_iter_kind,
        memo_slot,
        fold,
        map_body,
        acc,
        lit,
    };
    let outcome = fused.run(initial, ctx, engine, arena);
    // Everything from here stands in for dispatching the map node, which
    // adds the node to an error's breadcrumb.
    if outcome.is_err() {
        ctx.push_error_step(args[0].id());
    }
    outcome
}

/// A reduce-over-map the fusion accepted, with what the shape checks found.
struct FusedMap<'n> {
    map_args: &'n [CompiledNode],
    map_iter_kind: IterArgKind,
    memo_slot: Option<u16>,
    fold: FoldShape<'n>,
    map_body: FusedMapBody<'n>,
    /// The numeric initial value.
    acc: NumberValue,
    /// An `ArithVarLit` map body's literal, as a number.
    lit: NumberValue,
}

impl<'n> FusedMap<'n> {
    /// Resolve the map's input once, and either fold it or map it with the
    /// general `map` for the reduce to continue from.
    fn run<'a>(
        &self,
        initial: &'a DataValue<'a>,
        ctx: &mut ContextStack<'a>,
        engine: &Engine,
        arena: &'a Bump,
    ) -> Result<FusedOutcome<'a>>
    where
        'n: 'a,
    {
        // The map node's own operation, as dispatching it would charge.
        ctx.charge(1)?;
        let items =
            match resolve_iter_input(&self.map_args[0], self.map_iter_kind, ctx, engine, arena)? {
                ResolvedInput::Iterable(src) if !src.is_empty() => {
                    if let Some(value) =
                        run_fused_fold(&src, self.acc, self.lit, &self.fold, &self.map_body)
                    {
                        // Per item: the map body, the mapped item the reduce
                        // examines, and the fold body.
                        let per_item = self.map_body.body_cost() + 1 + FOLD_BODY_COST;
                        ctx.charge(per_item * src.len() as u64)?;
                        return Ok(FusedOutcome::Done(alloc_number(arena, value)));
                    }
                    Items::Array(src)
                }
                // `map` answers an empty source with `[]`, which the reduce
                // folds to its initial value.
                ResolvedInput::Iterable(_) | ResolvedInput::Empty => {
                    self.remember(crate::arena::singletons::singleton_empty_array(), ctx);
                    return Ok(FusedOutcome::Done(initial));
                }
                ResolvedInput::Bridge(DataValue::Object(pairs)) => Items::Object(pairs),
                ResolvedInput::Bridge(value) => Items::Scalar(value),
            };
        let mapped = super::map::evaluate_map(items, self.map_args, ctx, engine, arena)?;
        self.remember(mapped, ctx);
        Ok(FusedOutcome::Mapped(mapped))
    }

    /// Fill the CSE slot of a memoised map with its result, as dispatching
    /// the wrapper would have.
    fn remember<'a>(&self, mapped: &'a DataValue<'a>, ctx: &mut ContextStack<'a>) {
        if let Some(slot) = self.memo_slot {
            ctx.fill_cse_slot(slot, mapped);
        }
    }
}

/// The fused loop. Both the per-item map and the fold run through the
/// shared exact combine (`fused::combine`, via [`arith_number`] for the
/// map and `with_arith!` for the fold, whose operation and accumulator side
/// are fixed outside the loop), which applies the binary arithmetic
/// operators' own representation rules — so the fused result matches what the unfused
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
/// Anything non-numeric returns `None`, for the general `map` and the
/// reduce's own paths, which own coercion.
fn run_fused_fold(
    src: &IterSrc<'_>,
    mut acc: NumberValue,
    lit: NumberValue,
    fold: &FoldShape<'_>,
    map_body: &FusedMapBody<'_>,
) -> Option<NumberValue> {
    let op = fold.op;
    let acc_is_lhs = fold.acc_is_lhs;
    let mut cursors = FusedCursors::new(map_body);
    // The fold's operation and accumulator side are fixed outside the loop
    // (one loop per combination); the map body still dispatches per item.
    with_arith!(op, |f| {
        if acc_is_lhs {
            for item in src.0 {
                let mapped = mapped_number(map_body, &mut cursors, item, lit)?;
                acc = f(acc, mapped);
            }
        } else {
            for item in src.0 {
                let mapped = mapped_number(map_body, &mut cursors, item, lit)?;
                acc = f(mapped, acc);
            }
        }
    });
    Some(acc)
}

/// Allocate a fold result, short-circuiting to the preallocated small-int
/// singletons the way both fast paths did before.
#[inline(always)]
fn alloc_number<'a>(arena: &'a Bump, n: NumberValue) -> &'a DataValue<'a> {
    if let NumberValue::Integer(i) = n
        && let Some(singleton) = crate::arena::singletons::singleton_small_int(i)
    {
        return singleton;
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
            Some(arith_number(*op, x, y))
        }
        FusedMapBody::ArithVarVar { op, .. } => {
            let a = *cursors.a.resolve(item)?.as_number()?;
            let b = *cursors.b.as_mut()?.resolve(item)?.as_number()?;
            Some(arith_number(*op, a, b))
        }
    }
}

/// Detected `{+|-|*: [var, var]}` fold body over `current`/`accumulator`,
/// operand order preserved.
pub(super) struct FoldShape<'a> {
    op: ArithOp,
    /// true — body is `{op: [accumulator, current]}`; false — `[current, accumulator]`.
    acc_is_lhs: bool,
    /// Path below `current` (`"current.x.y"` → `["x", "y"]`); empty for bare `current`.
    current_segments: &'a [PathSegment],
}

/// Matches a reduce body of the shape `{+|-|*: [val("current"[+path]),
/// val("accumulator")]}` in either operand order, recording which side the
/// accumulator sits on so non-commutative folds evaluate correctly.
pub(super) fn detect_fold_shape(body: &CompiledNode) -> Option<FoldShape<'_>> {
    let CompiledNode::BuiltinOperator {
        opcode,
        args: body_args,
        ..
    } = body
    else {
        return None;
    };
    let op = opcode.arith_op()?;
    if body_args.len() != 2 {
        return None;
    }

    // Identify which arg is current and which is accumulator. The
    // accumulator must be read whole: the fold uses it as the running
    // number, so `accumulator.x` (null on a number, under the general
    // path) is left to the general path.
    let (current_arg, acc_is_lhs) = if is_bare_accumulator(&body_args[1]) {
        (&body_args[0], false)
    } else if is_bare_accumulator(&body_args[0]) {
        (&body_args[1], true)
    } else {
        return None;
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
        op,
        acc_is_lhs,
        current_segments,
    })
}

/// Whether `node` reads the whole accumulator: `{"var": "accumulator"}`,
/// or the same path spelled as a one-segment `val`.
fn is_bare_accumulator(node: &CompiledNode) -> bool {
    match node {
        CompiledNode::Var {
            reduce_hint: ReduceHint::Accumulator,
            ..
        } => true,
        CompiledNode::Var {
            reduce_hint: ReduceHint::AccumulatorPath,
            segments,
            ..
        } => segments.len() == 1,
        _ => false,
    }
}

/// Arena variant of the reduce arithmetic fast path: detects a `FoldShape`
/// body and folds without per-item context push or body dispatch. Iterates
/// `IterSrc` directly.
///
/// Folds through the shared exact combine for the same reason
/// [`run_fused_fold`] does: it applies the arithmetic operators'
/// representation rules, so the int/float decision at every step is
/// identical to general dispatch instead of mirrored. The operation and the
/// accumulator's side are fixed outside the loop (`with_arith!`): routing
/// both through a per-item dispatch made this loop's speed depend on code
/// placement. The previous two-pass form (exact i64 while it
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
    // One loop per (operation, accumulator side): the per-item step is
    // straight-line arithmetic, with no dispatch on either.
    with_arith!(op, |f| {
        if acc_is_lhs {
            for item in src.0 {
                acc = f(acc, *current_field.resolve(item)?.as_number()?);
            }
        } else {
            for item in src.0 {
                acc = f(*current_field.resolve(item)?.as_number()?, acc);
            }
        }
    });
    Some(alloc_number(arena, acc))
}
