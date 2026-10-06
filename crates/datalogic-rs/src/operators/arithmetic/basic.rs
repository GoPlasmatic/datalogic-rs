//! `+`, `-`, `*` — basic arithmetic with overflow promotion to `f64` and
//! optional datetime/duration support.

use crate::arena::{ContextStack, DataValue, coerce_to_number_cfg, try_coerce_to_integer_cfg};
use crate::operators::meta::ArithOp;
use crate::operators::{NanForm, nan_error};
use crate::{CompiledNode, Engine, Result};
use bumpalo::Bump;
use datavalue::NumberValue;

#[cfg(feature = "datetime")]
use super::helpers::fold_values;
use super::helpers::{
    FoldState, FoldStepOutcome, NanAction, VariadicFoldSpec, alloc_number, coerce_pair_f64,
    coerce_pair_int, handle_nan, is_literal_array, try_int_op, variadic_fold,
};

/// Arena-mode `+`. Handles 0-arg (identity), 1-arg array (sum elements),
/// 1-arg single value (coerce + return), 2-arg (numeric or datetime native),
/// and variadic (sum all args).
#[inline]
pub(crate) fn evaluate_add<'a>(
    args: &'a [CompiledNode],
    ctx: &mut ContextStack<'a>,
    engine: &Engine,
    arena: &'a Bump,
) -> Result<&'a DataValue<'a>> {
    if args.is_empty() {
        return Ok(alloc_number(arena, NumberValue::from_i64(0)));
    }
    if args.len() == 1 {
        return one_arg_arith(&args[0], ctx, engine, arena, ArithOp::Add);
    }
    if args.len() == 2 {
        return add_two_arg(&args[0], &args[1], ctx, engine, arena);
    }
    #[cfg(feature = "datetime")]
    if let Some(sum) = add_temporal_variadic(args, ctx, engine, arena)? {
        return Ok(sum);
    }
    variadic_fold(args, ctx, engine, arena, ADD_FOLD)
}

/// The numeric fold of a variadic `+`.
const ADD_FOLD: VariadicFoldSpec = VariadicFoldSpec {
    int_init: 0,
    float_init: 0.0,
    i_combine: i64::checked_add,
    f_combine: |a, b| a + b,
};

/// A variadic `+` whose first operand is a datetime or duration: the sum,
/// folded left to right as the two-argument `+` adds a pair, when every
/// operand is a datetime or duration and at most one is a datetime.
///
/// Returns `None` without evaluating anything when the first operand is
/// a number (or not temporal), for the numeric fold. When a later operand
/// breaks the sum, the operands so far are handed to the numeric fold,
/// which treats each as the non-numeric value it is (NaN handling).
#[cfg(feature = "datetime")]
fn add_temporal_variadic<'a>(
    args: &'a [CompiledNode],
    ctx: &mut ContextStack<'a>,
    engine: &Engine,
    arena: &'a Bump,
) -> Result<Option<&'a DataValue<'a>>> {
    use crate::operators::datetime::arith::Temporal;
    // Cheap pre-check on the node, so a numeric sum pays nothing: only an
    // operator call or a non-numeric literal can produce a temporal value.
    if let CompiledNode::Value { value, .. } = &args[0]
        && value.as_number().is_some()
    {
        return Ok(None);
    }
    let first = engine.dispatch_node(&args[0], ctx, arena)?;
    let numeric = first.as_i64().is_some() || coerce_to_number_cfg(first, engine).is_some();
    let mut acc = match (numeric, Temporal::of(first)) {
        (false, Some(t)) => t,
        _ => return numeric_fold_from(first, &args[1..], ctx, engine, arena, ADD_FOLD).map(Some),
    };
    for (i, arg) in args.iter().enumerate().skip(1) {
        let av = engine.dispatch_node(arg, ctx, arena)?;
        match Temporal::of(av).and_then(|t| acc.add(t)) {
            Some(sum) => acc = sum,
            None => {
                // Not a temporal sum: resume as the numeric fold, which
                // meets `args[..i]` as non-numeric operands, then `av`.
                let mut state = FoldState::new(ADD_FOLD.int_init, ADD_FOLD.float_init);
                for _ in 0..i {
                    if let FoldStepOutcome::ReturnNull = state.step(
                        None,
                        None,
                        ADD_FOLD.i_combine,
                        ADD_FOLD.f_combine,
                        ctx,
                        engine,
                    )? {
                        return Ok(Some(crate::arena::singletons::singleton_null()));
                    }
                }
                return fold_values(state, av, &args[i + 1..], ctx, engine, arena, ADD_FOLD)
                    .map(Some);
            }
        }
    }
    Ok(Some(acc.into_value(arena)))
}

/// [`variadic_fold`] with the first operand already evaluated.
#[cfg(feature = "datetime")]
fn numeric_fold_from<'a>(
    first: &'a DataValue<'a>,
    rest: &'a [CompiledNode],
    ctx: &mut ContextStack<'a>,
    engine: &Engine,
    arena: &'a Bump,
    spec: VariadicFoldSpec,
) -> Result<&'a DataValue<'a>> {
    let state = FoldState::new(spec.int_init, spec.float_init);
    fold_values(state, first, rest, ctx, engine, arena, spec)
}

#[inline]
fn add_two_arg<'a>(
    a: &'a CompiledNode,
    b: &'a CompiledNode,
    ctx: &mut ContextStack<'a>,
    engine: &Engine,
    arena: &'a Bump,
) -> Result<&'a DataValue<'a>> {
    let a_av = engine.dispatch_node(a, ctx, arena)?;
    let b_av = engine.dispatch_node(b, ctx, arena)?;

    // Integer-preserving fast path (both native Number with i64 values).
    if let (Some(ia), Some(ib)) = (a_av.as_i64(), b_av.as_i64()) {
        return Ok(alloc_number(
            arena,
            try_int_op(ia, ib, i64::checked_add, |x, y| x + y),
        ));
    }

    // Config-aware arena-native coercion (covers bool/null/string operands).
    if let Some((i1, i2)) = coerce_pair_int(a_av, b_av, engine) {
        return Ok(alloc_number(
            arena,
            try_int_op(i1, i2, i64::checked_add, |x, y| x + y),
        ));
    }
    if let Some((f1, f2)) = coerce_pair_f64(a_av, b_av, engine) {
        return Ok(alloc_number(arena, NumberValue::from_f64(f1 + f2)));
    }

    // Datetime / duration arithmetic.
    #[cfg(feature = "datetime")]
    {
        if let Some(av) = crate::operators::datetime::arith::datetime_add(a_av, b_av, arena) {
            return Ok(av);
        }
    }

    // Non-numeric, non-datetime — handle NaN per config.
    let mut sum = 0.0f64;
    for av in [a_av, b_av] {
        if let Some(f) = coerce_to_number_cfg(av, engine) {
            sum += f;
        } else {
            match handle_nan(ctx, engine)? {
                // Adding `0` leaves the sum as skipping does.
                NanAction::Skip | NanAction::Zero => {}
                NanAction::ReturnNull => return Ok(crate::arena::singletons::singleton_null()),
            }
        }
    }
    Ok(alloc_number(arena, NumberValue::from_f64(sum)))
}

/// Arena-mode `*`. 0-arg (1), 1-arg array (product), 1-arg scalar,
/// 2-arg (numeric or duration*scalar native), variadic.
#[inline]
pub(crate) fn evaluate_multiply<'a>(
    args: &'a [CompiledNode],
    ctx: &mut ContextStack<'a>,
    engine: &Engine,
    arena: &'a Bump,
) -> Result<&'a DataValue<'a>> {
    if args.is_empty() {
        return Ok(alloc_number(arena, NumberValue::from_i64(1)));
    }
    if args.len() == 1 {
        return one_arg_arith(&args[0], ctx, engine, arena, ArithOp::Mul);
    }
    if args.len() == 2 {
        return multiply_two_arg(&args[0], &args[1], ctx, engine, arena);
    }
    variadic_fold(
        args,
        ctx,
        engine,
        arena,
        VariadicFoldSpec {
            int_init: 1,
            float_init: 1.0,
            i_combine: i64::checked_mul,
            f_combine: |a, b| a * b,
        },
    )
}

#[inline]
fn multiply_two_arg<'a>(
    a: &'a CompiledNode,
    b: &'a CompiledNode,
    ctx: &mut ContextStack<'a>,
    engine: &Engine,
    arena: &'a Bump,
) -> Result<&'a DataValue<'a>> {
    let a_av = engine.dispatch_node(a, ctx, arena)?;
    let b_av = engine.dispatch_node(b, ctx, arena)?;

    // Integer-preserving fast path.
    if let (Some(ia), Some(ib)) = (a_av.as_i64(), b_av.as_i64()) {
        return Ok(alloc_number(
            arena,
            try_int_op(ia, ib, i64::checked_mul, |x, y| x * y),
        ));
    }

    // Duration * scalar — checked before generic coercion so duration object
    // inputs aren't coerced to None and lost.
    #[cfg(feature = "datetime")]
    {
        if let Some(av) = crate::operators::datetime::arith::datetime_multiply(a_av, b_av, arena) {
            return Ok(av);
        }
    }

    if let Some((i1, i2)) = coerce_pair_int(a_av, b_av, engine) {
        return Ok(alloc_number(
            arena,
            try_int_op(i1, i2, i64::checked_mul, |x, y| x * y),
        ));
    }
    if let Some((f1, f2)) = coerce_pair_f64(a_av, b_av, engine) {
        return Ok(alloc_number(arena, NumberValue::from_f64(f1 * f2)));
    }

    // Non-numeric — handle NaN per config (multiplicative identity is 1).
    let mut product = 1.0f64;
    for av in [a_av, b_av] {
        if let Some(f) = coerce_to_number_cfg(av, engine) {
            product *= f;
        } else {
            match handle_nan(ctx, engine)? {
                NanAction::Skip => {}
                NanAction::Zero => product = 0.0,
                NanAction::ReturnNull => return Ok(crate::arena::singletons::singleton_null()),
            }
        }
    }
    Ok(alloc_number(arena, NumberValue::from_f64(product)))
}

/// Arena-mode `-`. Handles 1-arg (negate / array fold), 2-arg primary
/// (numeric or datetime), and variadic (left-fold subtractive).
#[inline]
pub(crate) fn evaluate_subtract<'a>(
    args: &'a [CompiledNode],
    ctx: &mut ContextStack<'a>,
    engine: &Engine,
    arena: &'a Bump,
) -> Result<&'a DataValue<'a>> {
    if args.len() == 1 {
        return subtract_one_arg(&args[0], ctx, engine, arena);
    }
    if args.len() == 2 {
        return subtract_two_arg(&args[0], &args[1], ctx, engine, arena);
    }
    subtract_variadic(args, ctx, engine, arena)
}

#[inline]
fn subtract_one_arg<'a>(
    arg: &'a CompiledNode,
    ctx: &mut ContextStack<'a>,
    engine: &Engine,
    arena: &'a Bump,
) -> Result<&'a DataValue<'a>> {
    let av = engine.dispatch_node(arg, ctx, arena)?;

    // Array fold case: (first - second - ...). One per item, before the fold.
    if let DataValue::Array(items) = av {
        ctx.charge(items.len() as u64)?;
        if items.is_empty() {
            return Err(crate::Error::invalid_args());
        }
        let mut result = coerce_to_number_cfg(&items[0], engine)
            .ok_or_else(|| nan_error(NanForm::Thrown, ctx))?;
        for elem in &items[1..] {
            let n = coerce_to_number_cfg(elem, engine)
                .ok_or_else(|| nan_error(NanForm::Thrown, ctx))?;
            result -= n;
        }
        return Ok(alloc_number(arena, NumberValue::from_f64(result)));
    }
    // Negate single value (preserve integer typing when possible).
    if let Some(i) = av.as_i64() {
        return Ok(alloc_number(
            arena,
            i.checked_neg()
                .map(NumberValue::from_i64)
                .unwrap_or_else(|| NumberValue::from_f64(-(i as f64))),
        ));
    }
    if let Some(f) = coerce_to_number_cfg(av, engine) {
        return Ok(alloc_number(arena, NumberValue::from_f64(-f)));
    }
    Err(nan_error(NanForm::Thrown, ctx))
}

#[inline]
fn subtract_two_arg<'a>(
    a: &'a CompiledNode,
    b: &'a CompiledNode,
    ctx: &mut ContextStack<'a>,
    engine: &Engine,
    arena: &'a Bump,
) -> Result<&'a DataValue<'a>> {
    let a_av = engine.dispatch_node(a, ctx, arena)?;
    let b_av = engine.dispatch_node(b, ctx, arena)?;

    // Integer-preserving fast path.
    if let (Some(ia), Some(ib)) = (a_av.as_i64(), b_av.as_i64()) {
        return Ok(alloc_number(
            arena,
            try_int_op(ia, ib, i64::checked_sub, |x, y| x - y),
        ));
    }

    if let Some((i1, i2)) = coerce_pair_int(a_av, b_av, engine) {
        return Ok(alloc_number(
            arena,
            try_int_op(i1, i2, i64::checked_sub, |x, y| x - y),
        ));
    }
    if let Some((f1, f2)) = coerce_pair_f64(a_av, b_av, engine) {
        return Ok(alloc_number(arena, NumberValue::from_f64(f1 - f2)));
    }

    // Datetime / duration arithmetic.
    #[cfg(feature = "datetime")]
    {
        if let Some(av) = crate::operators::datetime::arith::datetime_subtract(a_av, b_av, arena) {
            return Ok(av);
        }
    }

    Err(nan_error(NanForm::Thrown, ctx))
}

/// Variadic (>2) subtract: integer fast path with overflow promotion.
///
/// Coercion strategy: `try_coerce_to_integer_cfg` (permissive) for the int
/// path so numeric strings stay on the int track; `coerce_to_number_cfg` for
/// the float path. The first arg seeds the accumulator and *must* coerce —
/// non-numeric first arg raises `NaN` immediately. Remaining args use the
/// usual NaN-handling config.
#[inline]
fn subtract_variadic<'a>(
    args: &'a [CompiledNode],
    ctx: &mut ContextStack<'a>,
    engine: &Engine,
    arena: &'a Bump,
) -> Result<&'a DataValue<'a>> {
    let first_av = engine.dispatch_node(&args[0], ctx, arena)?;
    let int_init = first_av
        .as_i64()
        .or_else(|| try_coerce_to_integer_cfg(first_av, engine));
    let float_init = match coerce_to_number_cfg(first_av, engine) {
        Some(f) => f,
        #[cfg(feature = "datetime")]
        None if let Some(first) = crate::operators::datetime::arith::Temporal::of(first_av) => {
            return subtract_temporal_variadic(first, &args[1..], ctx, engine, arena);
        }
        None => return Err(nan_error(NanForm::Thrown, ctx)),
    };
    let mut state = FoldState::new(int_init.unwrap_or_default(), float_init);
    state.all_int = int_init.is_some();

    for arg in args.iter().skip(1) {
        let av = engine.dispatch_node(arg, ctx, arena)?;
        let int_opt = av
            .as_i64()
            .or_else(|| try_coerce_to_integer_cfg(av, engine));
        let float_opt = if int_opt.is_some() {
            None
        } else {
            coerce_to_number_cfg(av, engine)
        };
        if let FoldStepOutcome::ReturnNull = state.step(
            int_opt,
            float_opt,
            i64::checked_sub,
            std::ops::Sub::sub,
            ctx,
            engine,
        )? {
            return Ok(crate::arena::singletons::singleton_null());
        }
    }
    Ok(state.finalize(arena))
}

/// A variadic `-` from a datetime or duration: folded left to right as the
/// two-argument `-` subtracts a pair. An operand the running value cannot
/// be reduced by is NaN, as any non-numeric start of a variadic `-` is.
#[cfg(feature = "datetime")]
fn subtract_temporal_variadic<'a>(
    first: crate::operators::datetime::arith::Temporal,
    rest: &'a [CompiledNode],
    ctx: &mut ContextStack<'a>,
    engine: &Engine,
    arena: &'a Bump,
) -> Result<&'a DataValue<'a>> {
    use crate::operators::datetime::arith::Temporal;
    let mut acc = first;
    for arg in rest {
        let av = engine.dispatch_node(arg, ctx, arena)?;
        acc = Temporal::of(av)
            .and_then(|t| acc.sub(t))
            .ok_or_else(|| nan_error(NanForm::Thrown, ctx))?;
    }
    Ok(acc.into_value(arena))
}

/// 1-arg `+` / `*`: literal-array reject, then either array-fold the elements
/// or treat as a single-value sum/product.
fn one_arg_arith<'a>(
    arg: &'a CompiledNode,
    ctx: &mut ContextStack<'a>,
    engine: &Engine,
    arena: &'a Bump,
    op: ArithOp,
) -> Result<&'a DataValue<'a>> {
    // Literal array argument is invalid for + / *. Apply NaN config (default
    // ThrowError → propagates the error up).
    if is_literal_array(arg) {
        return nan_operand(op, ctx, engine, arena);
    }

    let av = engine.dispatch_node(arg, ctx, arena)?;

    // Array result (e.g. from `var "items"`): fold all elements.
    if let DataValue::Array(items) = av {
        return one_arg_array_fold(items, ctx, engine, arena, op);
    }

    // Non-array single value: coerce and return (op identity * coerced).
    if let Some(i) = try_coerce_to_integer_cfg(av, engine) {
        return match op.checked_i64(op.right_identity(), i) {
            Some(r) => Ok(alloc_number(arena, NumberValue::from_i64(r))),
            None => Ok(alloc_number(
                arena,
                NumberValue::from_f64(op.apply_f64(op.right_identity() as f64, i as f64)),
            )),
        };
    }
    if let Some(f) = coerce_to_number_cfg(av, engine) {
        return Ok(alloc_number(
            arena,
            NumberValue::from_f64(op.apply_f64(op.right_identity() as f64, f)),
        ));
    }
    nan_operand(op, ctx, engine, arena)
}

/// The result of a one-operand `+` / `*` whose operand is not a number,
/// per the NaN config: the identity when it is skipped, `identity op 0`
/// (which is `0`) when it counts as zero.
fn nan_operand<'a>(
    op: ArithOp,
    ctx: &mut ContextStack<'a>,
    engine: &Engine,
    arena: &'a Bump,
) -> Result<&'a DataValue<'a>> {
    let value = match handle_nan(ctx, engine)? {
        NanAction::Skip => op.right_identity(),
        NanAction::Zero => op.checked_i64(op.right_identity(), 0).unwrap_or(0),
        NanAction::ReturnNull => return Ok(crate::arena::singletons::singleton_null()),
    };
    Ok(alloc_number(arena, NumberValue::from_i64(value)))
}

/// Fold an arena-resident array under `op` (`+` or `*`) with integer fast
/// path and overflow-to-f64.
///
/// Coercion strategy: `try_coerce_to_integer_cfg` for the int path (so
/// numeric-string elements stay on the int track), `coerce_to_number_cfg`
/// for the float fallback. Identical to `subtract_variadic`'s strategy
/// except the accumulator starts at `op.right_identity()` rather than
/// arg[0].
#[inline]
fn one_arg_array_fold<'a>(
    items: &[DataValue<'a>],
    ctx: &mut ContextStack<'a>,
    engine: &Engine,
    arena: &'a Bump,
    op: ArithOp,
) -> Result<&'a DataValue<'a>> {
    // One per item, before the fold: the node charge alone would price a
    // sum over a large array at a constant.
    ctx.charge(items.len() as u64)?;
    if items.is_empty() {
        return Ok(alloc_number(
            arena,
            NumberValue::from_i64(op.right_identity()),
        ));
    }
    let init = op.right_identity();
    let mut state = FoldState::new(init, init as f64);
    for item in items.iter() {
        let int_opt = try_coerce_to_integer_cfg(item, engine);
        let float_opt = if int_opt.is_some() {
            None
        } else {
            coerce_to_number_cfg(item, engine)
        };
        if let FoldStepOutcome::ReturnNull = state.step(
            int_opt,
            float_opt,
            |a, b| op.checked_i64(a, b),
            |a, b| op.apply_f64(a, b),
            ctx,
            engine,
        )? {
            return Ok(crate::arena::singletons::singleton_null());
        }
    }
    Ok(state.finalize(arena))
}
