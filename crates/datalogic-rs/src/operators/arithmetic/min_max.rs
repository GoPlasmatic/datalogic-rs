//! `min` and `max` — reductions over an array or variadic args. These are
//! "pipeline tops" that consume an array (typically produced by an upstream
//! filter/map). Arena wins:
//!   1. Input borrow: when args[0] is a root var, no clone of the input array.
//!   2. Composition: when args[0] is filter/map/all/some/none, the arena
//!      intermediate slice is consumed directly.
//!
//! Each op handles the SINGLE-ARG ARRAY form (e.g. `max(items)` over an array).
//! The multi-arg form (`max(a, b, c)`) is handled separately — it doesn't
//! involve array iteration.

use crate::arena::{ContextStack, DataValue};
use crate::operators::array::{IterArgKind, ResolvedInput, resolve_iter_input};
use crate::operators::meta::Extremum;
use crate::{CompiledNode, Engine, Result};
use bumpalo::Bump;

impl Extremum {
    /// The starting best: every number beats it.
    #[inline(always)]
    fn init(self) -> f64 {
        match self {
            Extremum::Max => f64::NEG_INFINITY,
            Extremum::Min => f64::INFINITY,
        }
    }

    /// Whether `candidate` strictly beats `best`, so the first of equal
    /// values wins.
    #[inline(always)]
    fn beats(self, candidate: f64, best: f64) -> bool {
        match self {
            Extremum::Max => candidate > best,
            Extremum::Min => candidate < best,
        }
    }
}

/// `max` / `min` over one array argument or several number arguments.
#[inline]
pub(crate) fn extremum<'a>(
    args: &'a [CompiledNode],
    iter_arg_kind: IterArgKind,
    ctx: &mut ContextStack<'a>,
    engine: &Engine,
    arena: &'a Bump,
    op: Extremum,
) -> Result<&'a DataValue<'a>> {
    // Multi-arg variadic form: evaluate each arg, pick the best Number.
    if args.len() > 1 {
        return extremum_variadic(args, ctx, engine, arena, op);
    }

    // Reject literal-array arg shape.
    if super::helpers::is_literal_array(&args[0]) {
        return Err(crate::Error::invalid_args());
    }

    let src = match resolve_iter_input(&args[0], iter_arg_kind, ctx, engine, arena)? {
        ResolvedInput::Iterable(s) => s,
        ResolvedInput::Empty => return Err(crate::Error::invalid_args()),
        ResolvedInput::Bridge(av) => {
            // Bridge is never Array/Null (see ResolvedInput::Bridge). A single
            // non-array arg must be a `Number`, returned unchanged.
            debug_assert!(!matches!(av, DataValue::Array(_) | DataValue::Null));
            if !matches!(av, DataValue::Number(_)) {
                return Err(crate::Error::invalid_args());
            }
            return Ok(av);
        }
    };

    if src.is_empty() {
        return Err(crate::Error::invalid_args());
    }

    let mut best_f = op.init();
    let mut best_idx: Option<usize> = None;
    let len = src.len();
    for i in 0..len {
        match src.get(i) {
            DataValue::Number(n) => {
                let f = n.as_f64();
                if op.beats(f, best_f) {
                    best_f = f;
                    best_idx = Some(i);
                }
            }
            _ => return Err(crate::Error::invalid_args()),
        }
    }

    match best_idx {
        // Re-borrow the arena value to preserve the original Number variant
        // (integer typing).
        Some(i) => Ok(src.get(i)),
        None => Ok(crate::arena::singletons::singleton_null()),
    }
}

#[inline]
fn extremum_variadic<'a>(
    args: &'a [CompiledNode],
    ctx: &mut ContextStack<'a>,
    engine: &Engine,
    arena: &'a Bump,
    op: Extremum,
) -> Result<&'a DataValue<'a>> {
    let mut best_f = op.init();
    let mut best_av: Option<&'a DataValue<'a>> = None;
    for arg in args {
        let av = engine.dispatch_node(arg, ctx, arena)?;
        let f = match av {
            DataValue::Number(n) => n.as_f64(),
            _ => return Err(crate::Error::invalid_args()),
        };
        if op.beats(f, best_f) {
            best_f = f;
            best_av = Some(av);
        }
    }
    match best_av {
        Some(av) => Ok(av),
        None => Ok(crate::arena::singletons::singleton_null()),
    }
}
