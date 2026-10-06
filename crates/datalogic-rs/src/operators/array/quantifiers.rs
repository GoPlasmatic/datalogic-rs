//! Quantifier operators: `all`, `some`, `none`.

use crate::arena::singletons::singleton_bool;
use crate::arena::{ContextStack, DataValue};
use crate::operators::meta::Quant;
use crate::{CompiledNode, Engine, Result};
use bumpalo::Bump;
use std::ops::ControlFlow;

use super::helpers::{FastPredicate, Items, for_each_iter_array, for_each_iter_object};

impl Quant {
    /// The predicate result that settles the answer early: `false` for
    /// `all`, `true` for `some` and `none`.
    #[inline(always)]
    fn short_circuit_on(self) -> bool {
        !matches!(self, Quant::All)
    }

    /// The answer for an empty collection. `all` is deliberately not
    /// vacuously true. A null or empty-array source is answered by the
    /// row's `on_empty_source`, which must agree; this covers an empty
    /// object and a scalar source.
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

/// `all` / `some` / `none`: test the predicate against each item (or
/// object pair), stopping at the first that settles the answer. A scalar
/// source counts as empty.
#[inline]
pub(crate) fn quantifier<'a>(
    items: Items<'a>,
    args: &'a [CompiledNode],
    ctx: &mut ContextStack<'a>,
    engine: &Engine,
    arena: &'a Bump,
    op: Quant,
) -> Result<&'a DataValue<'a>> {
    let predicate = &args[1];
    let src = match items {
        Items::Array(src) => src,
        Items::Object(pairs) => return quantifier_object(pairs, predicate, op, ctx, engine, arena),
        Items::Scalar(_) => return Ok(singleton_bool(op.empty_result())),
    };

    // Fast predicate path — no context push, no clones. Detection is
    // hoisted to compile time and cached on the predicate node, so we
    // pull it from there instead of pattern-matching every call. Skipped
    // when a tracer is attached so iteration markers still get recorded.
    // An indeterminate item (see `FastPredicate::evaluate_opt`) drops to
    // the general loop below, which is exact: fast evaluation is pure.
    if super::fast_paths::allowed(ctx, engine)
        && let Some(fast_pred) = FastPredicate::from_node(predicate)
    {
        let short_on = op.short_circuit_on();
        let mut found_short = false;
        let completed = fast_pred.scan(&src, engine, |_, hit| {
            if hit == short_on {
                found_short = true;
                ControlFlow::Break(())
            } else {
                ControlFlow::Continue(())
            }
        });
        if completed.is_some() {
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

/// An object source: the predicate runs per `(key, value)` pair.
#[inline]
fn quantifier_object<'a>(
    pairs: &'a [(&'a str, DataValue<'a>)],
    predicate: &'a CompiledNode,
    op: Quant,
    ctx: &mut ContextStack<'a>,
    engine: &Engine,
    arena: &'a Bump,
) -> Result<&'a DataValue<'a>> {
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
