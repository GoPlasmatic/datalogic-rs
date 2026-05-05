//! `/` and `%` — division and modulo. Shares the unified
//! [`div_or_mod`] entry point with a [`DivOp`] discriminator.

use crate::arena::{ContextStack, DataValue, coerce_to_number_cfg};
use crate::config::DivisionByZeroHandling;
use datavalue::NumberValue;
use crate::{CompiledNode, Engine, Result};
use bumpalo::Bump;

use super::helpers::alloc_number;

/// `/` vs `%` discriminant for the unified divide/modulo entry point.
#[derive(Clone, Copy)]
pub(crate) enum DivOp {
    Divide,
    Modulo,
}

impl DivOp {
    #[inline]
    fn apply_number(self, a: &NumberValue, b: &NumberValue) -> Option<NumberValue> {
        match self {
            DivOp::Divide => a.div(b),
            DivOp::Modulo => a.rem(b),
        }
    }

    #[inline]
    fn apply_f64(self, a: f64, b: f64) -> f64 {
        match self {
            DivOp::Divide => a / b,
            DivOp::Modulo => a % b,
        }
    }

    #[inline]
    fn is_modulo(self) -> bool {
        matches!(self, DivOp::Modulo)
    }
}

/// Native arena-mode `/` and `%`. Handles 1-arg array (fold), 1-arg scalar
/// (`/` → 1/x; `%` → invalid), 2-arg primary, variadic fold, and divbyzero
/// per engine config.
#[inline]
pub(crate) fn div_or_mod<'a>(
    args: &'a [CompiledNode],
    ctx: &mut ContextStack<'a>,
    engine: &Engine,
    arena: &'a Bump,
    op: DivOp,
) -> Result<&'a DataValue<'a>> {
    if args.is_empty() {
        return Err(crate::Error::invalid_args());
    }
    if args.len() == 1 {
        return one_arg_div_mod(&args[0], ctx, engine, arena, op);
    }
    if args.len() > 2 {
        return variadic_div_mod(args, ctx, engine, arena, op);
    }
    div_mod_two_arg(&args[0], &args[1], ctx, engine, arena, op)
}

#[inline]
fn div_mod_two_arg<'a>(
    a: &'a CompiledNode,
    b: &'a CompiledNode,
    ctx: &mut ContextStack<'a>,
    engine: &Engine,
    arena: &'a Bump,
    op: DivOp,
) -> Result<&'a DataValue<'a>> {
    let a_av = engine.dispatch_node(a, ctx, arena)?;
    let b_av = engine.dispatch_node(b, ctx, arena)?;

    // Duration / Number — only for `/`, not `%` (modulo on durations
    // is not defined).
    #[cfg(feature = "datetime")]
    if !op.is_modulo()
        && let Some(r) = super::datetime_arith::datetime_divide(a_av, b_av, arena)
    {
        return r;
    }

    let af = coerce_to_number_cfg(a_av, engine).ok_or_else(crate::Error::nan)?;
    let bf = coerce_to_number_cfg(b_av, engine).ok_or_else(crate::Error::nan)?;
    let na = NumberValue::from_f64(af);
    let nb = NumberValue::from_f64(bf);
    if nb.is_zero() {
        // Integer/integer with divisor=0 errors regardless of the
        // `division_by_zero` config (config only governs the float path).
        if a_av.as_i64().is_some() && b_av.as_i64().is_some() {
            return Err(crate::Error::nan());
        }
        return divbyzero(arena, na.as_f64(), engine);
    }
    match op.apply_number(&na, &nb) {
        Some(r) => Ok(alloc_number(arena, r)),
        None => Err(crate::Error::nan()),
    }
}

#[inline]
fn divbyzero<'a>(arena: &'a Bump, dividend: f64, engine: &Engine) -> Result<&'a DataValue<'a>> {
    match engine.config().division_by_zero {
        DivisionByZeroHandling::ThrowError => Err(crate::Error::nan()),
        DivisionByZeroHandling::ReturnNull => Ok(crate::arena::singletons::singleton_null()),
        DivisionByZeroHandling::ReturnInfinity => {
            let v = if dividend >= 0.0 {
                f64::INFINITY
            } else {
                f64::NEG_INFINITY
            };
            Ok(alloc_number(arena, NumberValue::from_f64(v)))
        }
        DivisionByZeroHandling::ReturnBounds => {
            let v = if dividend > 0.0 {
                f64::MAX
            } else if dividend < 0.0 {
                f64::MIN
            } else {
                0.0
            };
            Ok(alloc_number(arena, NumberValue::from_f64(v)))
        }
    }
}

/// 1-arg `/` and `%`:
///   * `/` with array → fold (a/b/c). `/` with non-array → 1/x.
///   * `%` with array of ≥2 numeric elements → fold (a%b%c). `%` with single
///     non-array argument → InvalidArguments.
fn one_arg_div_mod<'a>(
    arg: &'a CompiledNode,
    ctx: &mut ContextStack<'a>,
    engine: &Engine,
    arena: &'a Bump,
    op: DivOp,
) -> Result<&'a DataValue<'a>> {
    let av = engine.dispatch_node(arg, ctx, arena)?;

    if let DataValue::Array(items) = av {
        // Modulo requires ≥2 elements; divide tolerates 1+ (1-elem returns first).
        if items.is_empty() || (op.is_modulo() && items.len() < 2) {
            return Err(crate::Error::invalid_args());
        }
        let mut result = coerce_to_number_cfg(&items[0], engine).ok_or_else(crate::Error::nan)?;
        for elem in &items[1..] {
            let n = coerce_to_number_cfg(elem, engine).ok_or_else(crate::Error::nan)?;
            if n == 0.0 {
                return Err(crate::Error::nan());
            }
            result = op.apply_f64(result, n);
        }
        return Ok(alloc_number(arena, NumberValue::from_f64(result)));
    }

    // Non-array single value.
    if op.is_modulo() {
        return Err(crate::Error::invalid_args());
    }
    // 1/x with integer-preserving fast path.
    if let Some(i) = av.as_i64() {
        if i == 0 {
            return Err(crate::Error::nan());
        }
        if i == -1 {
            return Ok(alloc_number(arena, NumberValue::from_i64(-1)));
        }
        if 1 % i == 0 {
            return Ok(alloc_number(arena, NumberValue::from_i64(1 / i)));
        }
        return Ok(alloc_number(arena, NumberValue::from_f64(1.0 / i as f64)));
    }
    let f = coerce_to_number_cfg(av, engine).ok_or_else(crate::Error::nan)?;
    if f == 0.0 {
        return Err(crate::Error::nan());
    }
    Ok(alloc_number(arena, NumberValue::from_f64(1.0 / f)))
}

/// Native arena variadic (≥3 args) `/` / `%`. Folds left-associatively with
/// per-step zero-divisor check.
fn variadic_div_mod<'a>(
    args: &'a [CompiledNode],
    ctx: &mut ContextStack<'a>,
    engine: &Engine,
    arena: &'a Bump,
    op: DivOp,
) -> Result<&'a DataValue<'a>> {
    let first_av = engine.dispatch_node(&args[0], ctx, arena)?;
    let mut result = coerce_to_number_cfg(first_av, engine).ok_or_else(crate::Error::nan)?;
    for arg in args.iter().skip(1) {
        let av = engine.dispatch_node(arg, ctx, arena)?;
        let n = coerce_to_number_cfg(av, engine).ok_or_else(crate::Error::nan)?;
        if n == 0.0 {
            return Err(crate::Error::nan());
        }
        result = op.apply_f64(result, n);
    }
    Ok(alloc_number(arena, NumberValue::from_f64(result)))
}
