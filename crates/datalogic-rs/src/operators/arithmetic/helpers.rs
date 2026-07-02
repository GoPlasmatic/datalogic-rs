//! Shared infrastructure for the arithmetic operators: NaN handling,
//! coercion-pair helpers, the integer-checked-with-float-fallback pattern,
//! the variadic fold spec, and the small helpers around `DataValue::Number`
//! allocation.
//!
//! Coercion-pair helpers ([`coerce_pair_int`], [`coerce_pair_f64`]) are thin
//! wrappers over the engine-config-aware coercion in
//! [`crate::arena::coerce_to_number_cfg`] /
//! [`crate::arena::try_coerce_to_integer_cfg`]. See the module doc on
//! `src/arena/value/coercion.rs` for the full coercion-policy map across the
//! crate.

use crate::Engine;
use crate::Result;
use crate::arena::{DataValue, coerce_to_number_cfg, try_coerce_to_integer_cfg};
use crate::config::NanHandling;
use bumpalo::Bump;
use datavalue::NumberValue;

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
pub(super) fn handle_nan(engine: &Engine) -> Result<NanAction> {
    match engine.config().arithmetic_nan_handling {
        NanHandling::ThrowError => Err(crate::Error::nan()),
        NanHandling::IgnoreValue | NanHandling::CoerceToZero => Ok(NanAction::Skip),
        NanHandling::ReturnNull => Ok(NanAction::ReturnNull),
    }
}

/// Wrap a [`NumberValue`] in an arena-resident [`DataValue::Number`].
///
/// Note: we do *not* route through `singleton_small_int` here. Most arithmetic
/// results are not small non-negative integers, so probing the singleton table
/// on every alloc_number call costs more than it saves. Sites where the result
/// is *typically* small (length, var-index, reduce-int accumulator) call
/// `singleton_small_int` directly themselves.
#[inline]
pub(super) fn alloc_number<'a>(arena: &'a Bump, n: NumberValue) -> &'a DataValue<'a> {
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
    engine: &Engine,
) -> Option<(i64, i64)> {
    Some((
        try_coerce_to_integer_cfg(a, engine)?,
        try_coerce_to_integer_cfg(b, engine)?,
    ))
}

/// Coerce both operands to `f64` using the engine's config-aware coercion.
/// Returns `None` if either operand can't be coerced.
#[inline]
pub(super) fn coerce_pair_f64(
    a: &DataValue<'_>,
    b: &DataValue<'_>,
    engine: &Engine,
) -> Option<(f64, f64)> {
    Some((
        coerce_to_number_cfg(a, engine)?,
        coerce_to_number_cfg(b, engine)?,
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

/// Running int-fast-path / f64-fallback accumulator state.
///
/// The three arithmetic loops (`variadic_fold`, `subtract_variadic`,
/// `one_arg_array_fold`) used to inline the same `int_acc` /
/// `float_acc` / `all_int` advancement. They diverge on *coercion*
/// (whether to use `as_i64()` strict, `try_coerce_to_integer_cfg`
/// permissive, or both) but agree on what to do once coerced — that
/// shared portion lives here. Each caller picks its coercion strategy
/// and feeds [`Self::step`] pre-coerced `(int_opt, float_opt)`.
pub(super) struct FoldState {
    pub(super) int_acc: i64,
    pub(super) float_acc: f64,
    pub(super) all_int: bool,
}

/// Outcome of a single fold step.
pub(super) enum FoldStepOutcome {
    /// Accumulator advanced; continue the loop.
    Continue,
    /// Operand is non-numeric AND `arithmetic_nan_handling` is `ReturnNull`.
    /// Caller should bail out and return a Null arena value.
    ReturnNull,
}

impl FoldState {
    #[inline]
    pub(super) fn new(int_init: i64, float_init: f64) -> Self {
        Self {
            int_acc: int_init,
            float_acc: float_init,
            all_int: true,
        }
    }

    /// Advance the accumulator by one operand. `int_opt` and `float_opt`
    /// are the caller-coerced views of the operand (caller picks
    /// `as_i64()` / `try_coerce_to_integer_cfg` / `as_f64()` / etc.).
    /// `int_opt.is_some()` short-circuits the f64 path on the
    /// dominant int-only sequence; the caller pre-decides if int is the
    /// right interpretation. When both are `None`, NaN handling kicks
    /// in per the engine config.
    #[inline]
    pub(super) fn step<I, F>(
        &mut self,
        int_opt: Option<i64>,
        float_opt: Option<f64>,
        i_combine: I,
        f_combine: F,
        engine: &Engine,
    ) -> Result<FoldStepOutcome>
    where
        I: Fn(i64, i64) -> Option<i64>,
        F: Fn(f64, f64) -> f64,
    {
        if self.all_int {
            if let Some(i) = int_opt {
                match i_combine(self.int_acc, i) {
                    Some(r) => self.int_acc = r,
                    None => {
                        self.all_int = false;
                        self.float_acc = f_combine(self.int_acc as f64, i as f64);
                    }
                }
                return Ok(FoldStepOutcome::Continue);
            }
        }
        // `all_int` already flipped to `false` (a previous arg was non-int)
        // or `int_opt` is `None`. In the former case an integer arg still
        // needs to flow through the float accumulator.
        let float_arg = float_opt.or_else(|| int_opt.map(|i| i as f64));
        if let Some(f) = float_arg {
            if self.all_int {
                self.all_int = false;
                self.float_acc = f_combine(self.int_acc as f64, f);
            } else {
                self.float_acc = f_combine(self.float_acc, f);
            }
            return Ok(FoldStepOutcome::Continue);
        }
        match handle_nan(engine)? {
            NanAction::Skip => Ok(FoldStepOutcome::Continue),
            NanAction::ReturnNull => Ok(FoldStepOutcome::ReturnNull),
        }
    }

    /// Materialize the final accumulator as an arena `DataValue::Number`.
    #[inline]
    pub(super) fn finalize<'a>(self, arena: &'a Bump) -> &'a DataValue<'a> {
        if self.all_int {
            alloc_number(arena, NumberValue::from_i64(self.int_acc))
        } else {
            alloc_number(arena, NumberValue::from_f64(self.float_acc))
        }
    }
}

/// True when a compiled node is a literal array — either a structural
/// `Array` node or a `Value` wrapping an `OwnedDataValue::Array`. `+`/`*`
/// and `min`/`max` reject a single literal-array argument with this check.
#[inline]
pub(super) fn is_literal_array(node: &crate::CompiledNode) -> bool {
    matches!(node, crate::CompiledNode::Array { .. })
        || matches!(
            node,
            crate::CompiledNode::Value {
                value: datavalue::OwnedDataValue::Array(_),
                ..
            }
        )
}

/// Variadic fold over arena-evaluated args with integer-fast-path and
/// overflow promotion to `f64`. Used by `+` and `*` for the 2+ arg form.
/// Non-numeric args trigger NaN handling per engine config.
///
/// Coercion strategy: strict `as_i64()` for the int path; native `as_f64()`
/// then config-aware coercion for the float path. This is intentionally
/// stricter than `subtract_variadic` / `one_arg_array_fold` — variadic
/// `+`/`*` are dominated by native-int-only sequences and the strict path
/// avoids paying coercion cost on every arg.
#[inline]
pub(super) fn variadic_fold<'a>(
    args: &'a [crate::CompiledNode],
    ctx: &mut crate::arena::ContextStack<'a>,
    engine: &Engine,
    arena: &'a Bump,
    spec: VariadicFoldSpec,
) -> Result<&'a DataValue<'a>> {
    let mut state = FoldState::new(spec.int_init, spec.float_init);
    for arg in args {
        let av = engine.dispatch_node(arg, ctx, arena)?;
        let int_opt = av.as_i64();
        let float_opt = if int_opt.is_some() {
            None
        } else {
            av.as_f64().or_else(|| coerce_to_number_cfg(av, engine))
        };
        if let FoldStepOutcome::ReturnNull =
            state.step(int_opt, float_opt, spec.i_combine, spec.f_combine, engine)?
        {
            return Ok(crate::arena::singletons::singleton_null());
        }
    }
    Ok(state.finalize(arena))
}
