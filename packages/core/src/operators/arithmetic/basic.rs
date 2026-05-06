//! `+`, `-`, `*` — basic arithmetic with overflow promotion to `f64` and
//! optional datetime/duration support.

use crate::arena::{ContextStack, DataValue, coerce_to_number_cfg, try_coerce_to_integer_cfg};
use crate::{CompiledNode, Engine, Result};
use bumpalo::Bump;
use datavalue::NumberValue;

use super::helpers::{
    ArithOp, FoldState, FoldStepOutcome, NanAction, VariadicFoldSpec, alloc_number,
    coerce_pair_f64, coerce_pair_int, handle_nan, try_int_op, variadic_fold,
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
    variadic_fold(
        args,
        ctx,
        engine,
        arena,
        VariadicFoldSpec {
            int_init: 0,
            float_init: 0.0,
            i_combine: i64::checked_add,
            f_combine: |a, b| a + b,
        },
    )
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
        if let Some(av) = super::datetime_arith::datetime_add(a_av, b_av, arena) {
            return Ok(av);
        }
    }

    // Non-numeric, non-datetime — handle NaN per config.
    let mut sum = 0.0f64;
    for av in [a_av, b_av] {
        if let Some(f) = coerce_to_number_cfg(av, engine) {
            sum += f;
        } else {
            match handle_nan(engine)? {
                NanAction::Skip => {}
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
        return one_arg_arith(&args[0], ctx, engine, arena, ArithOp::Multiply);
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
        if let Some(av) = super::datetime_arith::datetime_multiply(a_av, b_av, arena) {
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
            match handle_nan(engine)? {
                NanAction::Skip => {}
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
    if args.is_empty() {
        return Err(crate::Error::invalid_args());
    }
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

    // Array fold case: (first - second - ...).
    if let DataValue::Array(items) = av {
        if items.is_empty() {
            return Err(crate::Error::invalid_args());
        }
        let mut result = coerce_to_number_cfg(&items[0], engine).ok_or_else(crate::Error::nan)?;
        for elem in &items[1..] {
            let n = coerce_to_number_cfg(elem, engine).ok_or_else(crate::Error::nan)?;
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
    Err(crate::Error::nan())
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
        if let Some(av) = super::datetime_arith::datetime_subtract(a_av, b_av, arena) {
            return Ok(av);
        }
    }

    Err(crate::Error::nan())
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
        None => return Err(crate::Error::nan()),
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
            engine,
        )? {
            return Ok(crate::arena::singletons::singleton_null());
        }
    }
    Ok(state.finalize(arena))
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
    let is_literal_array = matches!(arg, CompiledNode::Array { .. })
        || matches!(
            arg,
            CompiledNode::Value {
                value: datavalue::OwnedDataValue::Array(_),
                ..
            }
        );
    if is_literal_array {
        return match handle_nan(engine)? {
            NanAction::Skip => Ok(alloc_number(
                arena,
                NumberValue::from_i64(op.identity_int()),
            )),
            NanAction::ReturnNull => Ok(crate::arena::singletons::singleton_null()),
        };
    }

    let av = engine.dispatch_node(arg, ctx, arena)?;

    // Array result (e.g. from `var "items"`): fold all elements.
    if let DataValue::Array(items) = av {
        return one_arg_array_fold(items, engine, arena, op);
    }

    // Non-array single value: coerce and return (op identity * coerced).
    if let Some(i) = try_coerce_to_integer_cfg(av, engine) {
        return match op.combine_int(op.identity_int(), i) {
            Some(r) => Ok(alloc_number(arena, NumberValue::from_i64(r))),
            None => Ok(alloc_number(
                arena,
                NumberValue::from_f64(op.combine_f(op.identity_int() as f64, i as f64)),
            )),
        };
    }
    if let Some(f) = coerce_to_number_cfg(av, engine) {
        return Ok(alloc_number(
            arena,
            NumberValue::from_f64(op.combine_f(op.identity_int() as f64, f)),
        ));
    }
    match handle_nan(engine)? {
        NanAction::Skip => Ok(alloc_number(
            arena,
            NumberValue::from_i64(op.identity_int()),
        )),
        NanAction::ReturnNull => Ok(crate::arena::singletons::singleton_null()),
    }
}

/// Fold an arena-resident array under `op` (`+` or `*`) with integer fast
/// path and overflow-to-f64.
///
/// Coercion strategy: `try_coerce_to_integer_cfg` for the int path (so
/// numeric-string elements stay on the int track), `coerce_to_number_cfg`
/// for the float fallback. Identical to `subtract_variadic`'s strategy
/// except the accumulator starts at `op.identity_int()` rather than
/// arg[0].
#[inline]
fn one_arg_array_fold<'a>(
    items: &[DataValue<'a>],
    engine: &Engine,
    arena: &'a Bump,
    op: ArithOp,
) -> Result<&'a DataValue<'a>> {
    if items.is_empty() {
        return Ok(alloc_number(
            arena,
            NumberValue::from_i64(op.identity_int()),
        ));
    }
    let init = op.identity_int();
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
            |a, b| op.combine_int(a, b),
            |a, b| op.combine_f(a, b),
            engine,
        )? {
            return Ok(crate::arena::singletons::singleton_null());
        }
    }
    Ok(state.finalize(arena))
}
