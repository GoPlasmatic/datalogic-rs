//! Quantifier operators: `all`, `some`, `none`.

use crate::arena::singletons::singleton_bool;
use crate::arena::{ContextStack, DataValue};
use crate::operators::meta::Quant;
use crate::{CompiledNode, Engine, Result};
use bumpalo::Bump;
use std::ops::ControlFlow;

use super::helpers::{
    FastPredicate, IterArgKind, ResolvedInput, for_each_iter_array, for_each_iter_object,
    resolve_iter_input,
};

impl Quant {
    /// The predicate result that settles the answer early: `false` for
    /// `all`, `true` for `some` and `none`.
    #[inline(always)]
    fn short_circuit_on(self) -> bool {
        !matches!(self, Quant::All)
    }

    /// The answer for an empty collection. `all` is deliberately not
    /// vacuously true.
    #[inline(always)]
    fn empty_result(self) -> bool {
        matches!(self, Quant::None)
    }

    /// The answer, given whether some item hit [`Self::short_circuit_on`].
    #[inline(always)]
    fn finalize(self, found_short: bool) -> bool {
        match self {
            Quant::Some => found_short,
            Quant::All | Quant::None => !found_short,
        }
    }
}

/// `all` / `some` / `none`: test the predicate against each item, stopping
/// at the first that settles the answer.
#[inline]
pub(crate) fn quantifier<'a>(
    args: &'a [CompiledNode],
    iter_arg_kind: IterArgKind,
    ctx: &mut ContextStack<'a>,
    engine: &Engine,
    arena: &'a Bump,
    op: Quant,
) -> Result<&'a DataValue<'a>> {
    let predicate = &args[1];
    let src = match resolve_iter_input(&args[0], iter_arg_kind, ctx, engine, arena)? {
        ResolvedInput::Iterable(s) => s,
        ResolvedInput::Empty => return Ok(singleton_bool(op.empty_result())),
        ResolvedInput::Bridge(av) => {
            return quantifier_arena_bridge(av, predicate, op, ctx, engine, arena);
        }
    };

    if src.is_empty() {
        return Ok(singleton_bool(op.empty_result()));
    }

    // Fast predicate path — no context push, no clones. Detection is
    // hoisted to compile time and cached on the predicate node, so we
    // pull it from there instead of pattern-matching every call. Skipped
    // when a tracer is attached so iteration markers still get recorded.
    // An indeterminate item (see `FastPredicate::evaluate_opt`) drops to
    // the general loop below, which is exact: fast evaluation is pure.
    if !ctx.is_tracing()
        && let Some(fast_pred) = FastPredicate::from_node(predicate)
    {
        let len = src.len();
        let mut verdict = Some(false);
        for i in 0..len {
            match fast_pred.evaluate_opt(src.get(i), engine) {
                Some(hit) if hit == op.short_circuit_on() => {
                    verdict = Some(true);
                    break;
                }
                Some(_) => {}
                None => {
                    verdict = None;
                    break;
                }
            }
        }
        if let Some(found_short) = verdict {
            return Ok(singleton_bool(op.finalize(found_short)));
        }
    }

    // General path: zero-clone via ContextStack.
    let mut found_short = false;
    for_each_iter_array(src.0, predicate, ctx, engine, arena, |_, _item, av| {
        if crate::arena::truthy_arena(av, engine) == op.short_circuit_on() {
            found_short = true;
            return Ok(ControlFlow::Break(()));
        }
        Ok(ControlFlow::Continue(()))
    })?;
    Ok(singleton_bool(op.finalize(found_short)))
}

/// Quantifier Bridge case — Object inputs iterate (key, value) pairs. The
/// Bridge variant is only produced for non-null, non-array values
/// (`value_as_iter` routes Null to Empty and Array to Iterable), so every
/// other shape is treated as empty.
#[inline]
fn quantifier_arena_bridge<'a>(
    input: &'a DataValue<'a>,
    predicate: &'a CompiledNode,
    op: Quant,
    ctx: &mut ContextStack<'a>,
    engine: &Engine,
    arena: &'a Bump,
) -> Result<&'a DataValue<'a>> {
    match input {
        DataValue::Object(pairs) => {
            if pairs.is_empty() {
                return Ok(singleton_bool(op.empty_result()));
            }
            let mut found_short = false;
            for_each_iter_object(
                pairs,
                predicate,
                ctx,
                engine,
                arena,
                |_, _item, _key, av| {
                    if crate::arena::truthy_arena(av, engine) == op.short_circuit_on() {
                        found_short = true;
                        return Ok(ControlFlow::Break(()));
                    }
                    Ok(ControlFlow::Continue(()))
                },
            )?;
            Ok(singleton_bool(op.finalize(found_short)))
        }
        // Anything else (scalars, strings) — treated as empty. Null and Array
        // never reach the Bridge variant, so they are not handled here.
        _ => Ok(singleton_bool(op.empty_result())),
    }
}
