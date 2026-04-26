//! Quantifier operators: `all`, `some`, `none`.

use crate::arena::{ArenaContextStack, ArenaValue, IterGuard};
use crate::{CompiledNode, DataLogic, Result};
use bumpalo::Bump;

use super::helpers::{FastPredicate, ResolvedInput, resolve_iter_input};

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
fn evaluate_quantifier_arena<'a>(
    args: &'a [CompiledNode],
    actx: &mut ArenaContextStack<'a>,
    engine: &DataLogic,
    arena: &'a Bump,
    shape: QuantifierShape,
) -> Result<&'a ArenaValue<'a>> {
    if args.len() != 2 {
        return Err(crate::constants::invalid_args());
    }

    let predicate = &args[1];
    let src = match resolve_iter_input(&args[0], actx, engine, arena)? {
        ResolvedInput::Iterable(s) => s,
        ResolvedInput::Empty => return Ok(arena.alloc(ArenaValue::Bool(shape.empty_result))),
        ResolvedInput::Bridge(av) => {
            return quantifier_arena_bridge(av, predicate, shape, actx, engine, arena);
        }
    };

    if src.is_empty() {
        return Ok(arena.alloc(ArenaValue::Bool(shape.empty_result)));
    }

    // Fast predicate path — no context push, no clones.
    if let Some(fast_pred) = FastPredicate::try_detect(predicate) {
        let len = src.len();
        for i in 0..len {
            if fast_pred.evaluate(src.get(i), arena) == shape.short_circuit_on {
                return Ok(arena.alloc(ArenaValue::Bool(shape.finalize(true))));
            }
        }
        return Ok(arena.alloc(ArenaValue::Bool(shape.finalize(false))));
    }

    // General path: zero-clone via ArenaContextStack.
    let len = src.len();
    let total = len as u32;
    let mut found_short = false;
    let mut guard = IterGuard::new(actx);
    for i in 0..len {
        let item = src.get(i);
        guard.step_indexed(item, i);
        let av = engine.eval_iter_body(predicate, guard.stack(), arena, i as u32, total)?;
        if crate::arena::is_truthy_arena(av, engine) == shape.short_circuit_on {
            found_short = true;
            break;
        }
    }
    drop(guard);
    Ok(arena.alloc(ArenaValue::Bool(shape.finalize(found_short))))
}

/// Quantifier Bridge case — Object inputs iterate (key, value) pairs;
/// inline arena Array inputs iterate items.
#[inline]
fn quantifier_arena_bridge<'a>(
    input: &'a ArenaValue<'a>,
    predicate: &'a CompiledNode,
    shape: QuantifierShape,
    actx: &mut ArenaContextStack<'a>,
    engine: &DataLogic,
    arena: &'a Bump,
) -> Result<&'a ArenaValue<'a>> {
    match input {
        ArenaValue::Object(pairs) => {
            if pairs.is_empty() {
                return Ok(arena.alloc(ArenaValue::Bool(shape.empty_result)));
            }
            let total = pairs.len() as u32;
            let mut found_short = false;
            let mut guard = IterGuard::new(actx);
            for (i, (k, v)) in pairs.iter().enumerate() {
                // SAFETY: pairs[i].1 lives in the arena for `'a`.
                let item_av: &'a ArenaValue<'a> = unsafe { &*(v as *const ArenaValue<'a>) };
                let key_arena: &'a str = k;
                guard.step_keyed(item_av, i, key_arena);
                let av = engine.eval_iter_body(predicate, guard.stack(), arena, i as u32, total)?;
                if crate::arena::is_truthy_arena(av, engine) == shape.short_circuit_on {
                    found_short = true;
                    break;
                }
            }
            drop(guard);
            Ok(arena.alloc(ArenaValue::Bool(shape.finalize(found_short))))
        }
        ArenaValue::Array(items) => {
            if items.is_empty() {
                return Ok(arena.alloc(ArenaValue::Bool(shape.empty_result)));
            }
            let total = items.len() as u32;
            let mut found_short = false;
            let mut guard = IterGuard::new(actx);
            for (i, item_av) in items.iter().enumerate() {
                guard.step_indexed(item_av, i);
                let av = engine.eval_iter_body(predicate, guard.stack(), arena, i as u32, total)?;
                if crate::arena::is_truthy_arena(av, engine) == shape.short_circuit_on {
                    found_short = true;
                    break;
                }
            }
            drop(guard);
            Ok(arena.alloc(ArenaValue::Bool(shape.finalize(found_short))))
        }
        // Anything else — treated as empty (returns empty_result).
        _ => Ok(arena.alloc(ArenaValue::Bool(shape.empty_result))),
    }
}

/// Arena-mode `all` — true iff every item satisfies predicate. Short-circuits on false.
#[inline]
pub(crate) fn evaluate_all_arena<'a>(
    args: &'a [CompiledNode],
    actx: &mut ArenaContextStack<'a>,
    engine: &DataLogic,
    arena: &'a Bump,
) -> Result<&'a ArenaValue<'a>> {
    // all: early-exit on false; empty array ⇒ false (matching existing impl,
    // which deliberately rejects vacuous truth).
    evaluate_quantifier_arena(
        args,
        actx,
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
pub(crate) fn evaluate_some_arena<'a>(
    args: &'a [CompiledNode],
    actx: &mut ArenaContextStack<'a>,
    engine: &DataLogic,
    arena: &'a Bump,
) -> Result<&'a ArenaValue<'a>> {
    // some: early-exit on true; empty array ⇒ false.
    evaluate_quantifier_arena(
        args,
        actx,
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
pub(crate) fn evaluate_none_arena<'a>(
    args: &'a [CompiledNode],
    actx: &mut ArenaContextStack<'a>,
    engine: &DataLogic,
    arena: &'a Bump,
) -> Result<&'a ArenaValue<'a>> {
    // none: early-exit on true (then return false); empty array ⇒ true.
    evaluate_quantifier_arena(
        args,
        actx,
        engine,
        arena,
        QuantifierShape {
            short_circuit_on: true,
            invert_final: true,
            empty_result: true,
        },
    )
}
