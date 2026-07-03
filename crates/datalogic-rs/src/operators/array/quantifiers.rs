//! Quantifier operators: `all`, `some`, `none`.

use crate::arena::singletons::singleton_bool;
use crate::arena::{ContextStack, DataValue};
use crate::{CompiledNode, Engine, Result};
use bumpalo::Bump;
use std::ops::ControlFlow;

use super::helpers::{
    FastPredicate, IterArgKind, ResolvedInput, for_each_iter_array, for_each_iter_object,
    resolve_iter_input,
};

/// Shape of a quantifier (`all` / `some` / `none`) — the three flags
/// distinguishing them are bundled here so callers and helpers don't carry
/// three loose `bool` parameters.
#[derive(Clone, Copy)]
pub(super) struct QuantifierShape {
    /// Predicate result that triggers early exit.
    pub(super) short_circuit_on: bool,
    /// If `true`, invert `short_circuit_on` when assembling the final result.
    pub(super) invert_final: bool,
    /// Result for an empty input collection.
    pub(super) empty_result: bool,
}

impl QuantifierShape {
    #[inline]
    fn finalize(self, found_short: bool) -> bool {
        if found_short {
            if self.invert_final {
                !self.short_circuit_on
            } else {
                self.short_circuit_on
            }
        } else if self.invert_final {
            self.short_circuit_on
        } else {
            !self.short_circuit_on
        }
    }
}

/// Internal helper: arena-mode quantifier (all / some / none).
/// `early_truthy` controls short-circuit semantics:
///   - `all`: early_truthy = false (false ⇒ return false immediately)
///   - `some`: early_truthy = true (true ⇒ return true immediately)
///   - `none`: same as `some` but invert the final result
#[inline]
fn evaluate_quantifier<'a>(
    args: &'a [CompiledNode],
    iter_arg_kind: IterArgKind,
    ctx: &mut ContextStack<'a>,
    engine: &Engine,
    arena: &'a Bump,
    shape: QuantifierShape,
) -> Result<&'a DataValue<'a>> {
    if args.len() != 2 {
        return Err(crate::Error::invalid_args());
    }

    let predicate = &args[1];
    let src = match resolve_iter_input(&args[0], iter_arg_kind, ctx, engine, arena)? {
        ResolvedInput::Iterable(s) => s,
        ResolvedInput::Empty => return Ok(singleton_bool(shape.empty_result)),
        ResolvedInput::Bridge(av) => {
            return quantifier_arena_bridge(av, predicate, shape, ctx, engine, arena);
        }
    };

    if src.is_empty() {
        return Ok(singleton_bool(shape.empty_result));
    }

    // Fast predicate path — no context push, no clones. Detection is
    // hoisted to compile time and cached on the predicate node, so we
    // pull it from there instead of pattern-matching every call. Skipped
    // when a tracer is attached so iteration markers still get recorded.
    // An indeterminate item (see `FastPredicate::evaluate_opt`) drops to
    // the general loop below, which is exact: fast evaluation is pure.
    if !ctx.is_tracing() {
        if let Some(fast_pred) = FastPredicate::from_node(predicate) {
            let len = src.len();
            let mut verdict = Some(false);
            for i in 0..len {
                match fast_pred.evaluate_opt(src.get(i), engine) {
                    Some(hit) if hit == shape.short_circuit_on => {
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
                return Ok(singleton_bool(shape.finalize(found_short)));
            }
        }
    }

    // General path: zero-clone via ContextStack.
    let mut found_short = false;
    for_each_iter_array(src.0, predicate, ctx, engine, arena, |_, _item, av| {
        if crate::arena::truthy_arena(av, engine) == shape.short_circuit_on {
            found_short = true;
            return Ok(ControlFlow::Break(()));
        }
        Ok(ControlFlow::Continue(()))
    })?;
    Ok(singleton_bool(shape.finalize(found_short)))
}

/// Quantifier Bridge case — Object inputs iterate (key, value) pairs. The
/// Bridge variant is only produced for non-null, non-array values
/// (`value_as_iter` routes Null to Empty and Array to Iterable), so every
/// other shape is treated as empty.
#[inline]
fn quantifier_arena_bridge<'a>(
    input: &'a DataValue<'a>,
    predicate: &'a CompiledNode,
    shape: QuantifierShape,
    ctx: &mut ContextStack<'a>,
    engine: &Engine,
    arena: &'a Bump,
) -> Result<&'a DataValue<'a>> {
    match input {
        DataValue::Object(pairs) => {
            if pairs.is_empty() {
                return Ok(singleton_bool(shape.empty_result));
            }
            let mut found_short = false;
            for_each_iter_object(
                pairs,
                predicate,
                ctx,
                engine,
                arena,
                |_, _item, _key, av| {
                    if crate::arena::truthy_arena(av, engine) == shape.short_circuit_on {
                        found_short = true;
                        return Ok(ControlFlow::Break(()));
                    }
                    Ok(ControlFlow::Continue(()))
                },
            )?;
            Ok(singleton_bool(shape.finalize(found_short)))
        }
        // Anything else (scalars, strings) — treated as empty. Null and Array
        // never reach the Bridge variant, so they are not handled here.
        _ => Ok(singleton_bool(shape.empty_result)),
    }
}

/// Arena-mode `all` — true iff every item satisfies predicate. Short-circuits on false.
#[inline]
pub(crate) fn evaluate_all<'a>(
    args: &'a [CompiledNode],
    iter_arg_kind: IterArgKind,
    ctx: &mut ContextStack<'a>,
    engine: &Engine,
    arena: &'a Bump,
) -> Result<&'a DataValue<'a>> {
    // all: early-exit on false; empty array ⇒ false (matching existing impl,
    // which deliberately rejects vacuous truth).
    evaluate_quantifier(
        args,
        iter_arg_kind,
        ctx,
        engine,
        arena,
        QuantifierShape {
            short_circuit_on: false,
            invert_final: false,
            empty_result: false,
        },
    )
}

/// Arena-mode `some` — true iff any item satisfies predicate. Short-circuits on true.
#[inline]
pub(crate) fn evaluate_some<'a>(
    args: &'a [CompiledNode],
    iter_arg_kind: IterArgKind,
    ctx: &mut ContextStack<'a>,
    engine: &Engine,
    arena: &'a Bump,
) -> Result<&'a DataValue<'a>> {
    // some: early-exit on true; empty array ⇒ false.
    evaluate_quantifier(
        args,
        iter_arg_kind,
        ctx,
        engine,
        arena,
        QuantifierShape {
            short_circuit_on: true,
            invert_final: false,
            empty_result: false,
        },
    )
}

/// Arena-mode `none` — true iff no item satisfies predicate. Short-circuits on true.
#[inline]
pub(crate) fn evaluate_none<'a>(
    args: &'a [CompiledNode],
    iter_arg_kind: IterArgKind,
    ctx: &mut ContextStack<'a>,
    engine: &Engine,
    arena: &'a Bump,
) -> Result<&'a DataValue<'a>> {
    // none: early-exit on true (then return false); empty array ⇒ true.
    evaluate_quantifier(
        args,
        iter_arg_kind,
        ctx,
        engine,
        arena,
        QuantifierShape {
            short_circuit_on: true,
            invert_final: true,
            empty_result: true,
        },
    )
}
