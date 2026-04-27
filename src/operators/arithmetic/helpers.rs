//! Shared infrastructure for the arithmetic operators: NaN handling,
//! coercion-pair helpers, the integer-checked-with-float-fallback pattern,
//! the variadic fold spec, and the small helpers around `DataValue::Number`
//! allocation.

use crate::DataLogic;
use crate::Result;
use crate::arena::{DataValue, coerce_arena_to_number_cfg, try_coerce_arena_to_integer_cfg};
use crate::config::NanHandling;
use crate::value::NumberValue;
use bumpalo::Bump;

/// Result of NaN handling check: what the caller should do with a non-numeric value.
pub(super) enum NanAction {
    /// Skip/ignore this value (`IgnoreValue` or `CoerceToZero`).
    Skip,
    /// Return null immediately.
    ReturnNull,
}

/// Check the engine's NaN handling config and return the appropriate action.
/// Returns `Err` for `ThrowError`, `Ok(NanAction)` otherwise.
#[inline]
pub(super) fn handle_nan(engine: &DataLogic) -> Result<NanAction> {
    match engine.config().arithmetic_nan_handling {
        NanHandling::ThrowError => Err(crate::constants::nan_error()),
        NanHandling::IgnoreValue | NanHandling::CoerceToZero => Ok(NanAction::Skip),
        NanHandling::ReturnNull => Ok(NanAction::ReturnNull),
    }
}

/// Wrap a [`NumberValue`] in an arena-resident [`DataValue::Number`].
#[inline]
pub(super) fn arena_number<'a>(arena: &'a Bump, n: NumberValue) -> &'a DataValue<'a> {
    arena.alloc(DataValue::Number(n))
}

/// Try the checked-integer op; on overflow promote both operands to `f64` and
/// apply the float fallback. Collapses the recurring
/// `match a.checked_op(b) { Some(r) => from_i64(r), None => from_f64(...) }`
/// pattern that appears across `+`, `-`, and `*`.
#[inline]
pub(super) fn try_int_op(
    a: i64,
    b: i64,
    int_op: fn(i64, i64) -> Option<i64>,
    float_op: fn(f64, f64) -> f64,
) -> NumberValue {
    match int_op(a, b) {
        Some(r) => NumberValue::from_i64(r),
        None => NumberValue::from_f64(float_op(a as f64, b as f64)),
    }
}

/// Coerce both operands to `i64` using the engine's config-aware coercion.
/// Returns `None` if either operand can't be coerced.
#[inline]
pub(super) fn coerce_pair_int(
    a: &DataValue<'_>,
    b: &DataValue<'_>,
    engine: &DataLogic,
) -> Option<(i64, i64)> {
    Some((
        try_coerce_arena_to_integer_cfg(a, engine)?,
        try_coerce_arena_to_integer_cfg(b, engine)?,
    ))
}

/// Coerce both operands to `f64` using the engine's config-aware coercion.
/// Returns `None` if either operand can't be coerced.
#[inline]
pub(super) fn coerce_pair_f64(
    a: &DataValue<'_>,
    b: &DataValue<'_>,
    engine: &DataLogic,
) -> Option<(f64, f64)> {
    Some((
        coerce_arena_to_number_cfg(a, engine)?,
        coerce_arena_to_number_cfg(b, engine)?,
    ))
}

/// Operation discriminator for the shared 1-arg fold (`+` and `*`).
#[derive(Clone, Copy)]
pub(super) enum ArithOp {
    Add,
    Multiply,
}

impl ArithOp {
    #[inline]
    pub(super) fn identity_int(self) -> i64 {
        match self {
            ArithOp::Add => 0,
            ArithOp::Multiply => 1,
        }
    }

    #[inline]
    pub(super) fn combine_int(self, a: i64, b: i64) -> Option<i64> {
        match self {
            ArithOp::Add => a.checked_add(b),
            ArithOp::Multiply => a.checked_mul(b),
        }
    }

    #[inline]
    pub(super) fn combine_f(self, a: f64, b: f64) -> f64 {
        match self {
            ArithOp::Add => a + b,
            ArithOp::Multiply => a * b,
        }
    }
}

/// Spec for an integer-fast-path / float-fallback variadic fold:
/// inits, the integer combine (with overflow signaling via `None`), and
/// the float combine.
pub(super) struct VariadicFoldSpec {
    pub(super) int_init: i64,
    pub(super) float_init: f64,
    pub(super) i_combine: fn(i64, i64) -> Option<i64>,
    pub(super) f_combine: fn(f64, f64) -> f64,
}

/// Variadic fold over arena-evaluated args with integer-fast-path and
/// overflow promotion to `f64`. Used by `+` and `*` for the 2+ arg form.
/// Non-numeric args trigger NaN handling per engine config.
#[inline]
pub(super) fn arena_variadic_fold<'a>(
    args: &'a [crate::CompiledNode],
    actx: &mut crate::arena::DataContextStack<'a>,
    engine: &DataLogic,
    arena: &'a Bump,
    spec: VariadicFoldSpec,
) -> Result<&'a DataValue<'a>> {
    let mut int_acc: i64 = spec.int_init;
    let mut float_acc: f64 = spec.float_init;
    let mut all_int = true;

    for arg in args {
        let av = engine.evaluate_node(arg, actx, arena)?;
        if all_int && let Some(i) = av.as_i64() {
            match (spec.i_combine)(int_acc, i) {
                Some(r) => int_acc = r,
                None => {
                    all_int = false;
                    float_acc = (spec.f_combine)(int_acc as f64, i as f64);
                }
            }
            continue;
        }
        // Try `as_f64` for native numbers first; fall back to coercion so
        // `true`/`false`/`null`/numeric strings compose into the variadic op.
        let f_opt = av
            .as_f64()
            .or_else(|| coerce_arena_to_number_cfg(av, engine));
        if let Some(f) = f_opt {
            if all_int {
                all_int = false;
                float_acc = (spec.f_combine)(int_acc as f64, f);
            } else {
                float_acc = (spec.f_combine)(float_acc, f);
            }
        } else {
            // Non-numeric operand — variadic (>2) `+`/`*` treats
            // arrays/objects/non-coercibles as NaN per `arithmetic_nan_handling`.
            match handle_nan(engine)? {
                NanAction::Skip => continue,
                NanAction::ReturnNull => {
                    return Ok(crate::arena::pool::singleton_null());
                }
            }
        }
    }

    if all_int {
        Ok(arena_number(arena, NumberValue::from_i64(int_acc)))
    } else {
        Ok(arena_number(arena, NumberValue::from_f64(float_acc)))
    }
}
