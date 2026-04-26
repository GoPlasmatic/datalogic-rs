//! `+`, `-`, `*` — basic arithmetic with overflow promotion to `f64` and
//! optional datetime/duration support.

use serde_json::Value;

use crate::arena::{
    ArenaContextStack, ArenaValue, coerce_arena_to_number_cfg, try_coerce_arena_to_integer_cfg,
};
use crate::value::NumberValue;
use crate::{CompiledNode, DataLogic, Result};
use bumpalo::Bump;

use super::helpers::{
    ArithOp, NanAction, VariadicFoldSpec, arena_number, arena_variadic_fold, coerce_pair_f64,
    coerce_pair_int, handle_nan, try_int_op,
};

/// Arena-mode `+`. Handles 0-arg (identity), 1-arg array (sum elements),
/// 1-arg single value (coerce + return), 2-arg (numeric or datetime native),
/// and variadic (sum all args).
#[inline]
pub(crate) fn evaluate_add_arena<'a>(
    args: &'a [CompiledNode],
    actx: &mut ArenaContextStack<'a>,
    engine: &DataLogic,
    arena: &'a Bump,
) -> Result<&'a ArenaValue<'a>> {
    if args.is_empty() {
        return Ok(arena_number(arena, NumberValue::from_i64(0)));
    }
    if args.len() == 1 {
        return arena_one_arg_arith(&args[0], actx, engine, arena, ArithOp::Add);
    }
    if args.len() == 2 {
        return add_two_arg(&args[0], &args[1], actx, engine, arena);
    }
    arena_variadic_fold(
        args,
        actx,
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
    actx: &mut ArenaContextStack<'a>,
    engine: &DataLogic,
    arena: &'a Bump,
) -> Result<&'a ArenaValue<'a>> {
    let a_av = engine.evaluate_arena_node(a, actx, arena)?;
    let b_av = engine.evaluate_arena_node(b, actx, arena)?;

    // Integer-preserving fast path (both native Number with i64 values).
    if let (Some(ia), Some(ib)) = (a_av.as_i64(), b_av.as_i64()) {
        return Ok(arena_number(
            arena,
            try_int_op(ia, ib, i64::checked_add, |x, y| x + y),
        ));
    }

    // Config-aware arena-native coercion (covers bool/null/string operands).
    if let Some((i1, i2)) = coerce_pair_int(a_av, b_av, engine) {
        return Ok(arena_number(
            arena,
            try_int_op(i1, i2, i64::checked_add, |x, y| x + y),
        ));
    }
    if let Some((f1, f2)) = coerce_pair_f64(a_av, b_av, engine) {
        return Ok(arena_number(arena, NumberValue::from_f64(f1 + f2)));
    }

    // Datetime / duration arithmetic.
    #[cfg(feature = "datetime")]
    {
        if let Some(av) = super::datetime_arith::arena_datetime_add(a_av, b_av, arena) {
            return Ok(av);
        }
    }

    // Non-numeric, non-datetime — handle NaN per config.
    let mut sum = 0.0f64;
    for av in [a_av, b_av] {
        if let Some(f) = coerce_arena_to_number_cfg(av, engine) {
            sum += f;
        } else {
            match handle_nan(engine)? {
                NanAction::Skip => {}
                NanAction::ReturnNull => return Ok(crate::arena::pool::singleton_null()),
            }
        }
    }
    Ok(arena_number(arena, NumberValue::from_f64(sum)))
}

/// Arena-mode `*`. 0-arg (1), 1-arg array (product), 1-arg scalar,
/// 2-arg (numeric or duration*scalar native), variadic.
#[inline]
pub(crate) fn evaluate_multiply_arena<'a>(
    args: &'a [CompiledNode],
    actx: &mut ArenaContextStack<'a>,
    engine: &DataLogic,
    arena: &'a Bump,
) -> Result<&'a ArenaValue<'a>> {
    if args.is_empty() {
        return Ok(arena_number(arena, NumberValue::from_i64(1)));
    }
    if args.len() == 1 {
        return arena_one_arg_arith(&args[0], actx, engine, arena, ArithOp::Multiply);
    }
    if args.len() == 2 {
        return multiply_two_arg(&args[0], &args[1], actx, engine, arena);
    }
    arena_variadic_fold(
        args,
        actx,
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
    actx: &mut ArenaContextStack<'a>,
    engine: &DataLogic,
    arena: &'a Bump,
) -> Result<&'a ArenaValue<'a>> {
    let a_av = engine.evaluate_arena_node(a, actx, arena)?;
    let b_av = engine.evaluate_arena_node(b, actx, arena)?;

    // Integer-preserving fast path.
    if let (Some(ia), Some(ib)) = (a_av.as_i64(), b_av.as_i64()) {
        return Ok(arena_number(
            arena,
            try_int_op(ia, ib, i64::checked_mul, |x, y| x * y),
        ));
    }

    // Duration * scalar — checked before generic coercion so duration object
    // inputs aren't coerced to None and lost.
    #[cfg(feature = "datetime")]
    {
        if let Some(av) = super::datetime_arith::arena_datetime_multiply(a_av, b_av, arena) {
            return Ok(av);
        }
    }

    if let Some((i1, i2)) = coerce_pair_int(a_av, b_av, engine) {
        return Ok(arena_number(
            arena,
            try_int_op(i1, i2, i64::checked_mul, |x, y| x * y),
        ));
    }
    if let Some((f1, f2)) = coerce_pair_f64(a_av, b_av, engine) {
        return Ok(arena_number(arena, NumberValue::from_f64(f1 * f2)));
    }

    // Non-numeric — handle NaN per config (multiplicative identity is 1).
    let mut product = 1.0f64;
    for av in [a_av, b_av] {
        if let Some(f) = coerce_arena_to_number_cfg(av, engine) {
            product *= f;
        } else {
            match handle_nan(engine)? {
                NanAction::Skip => {}
                NanAction::ReturnNull => return Ok(crate::arena::pool::singleton_null()),
            }
        }
    }
    Ok(arena_number(arena, NumberValue::from_f64(product)))
}

/// Arena-mode `-`. Handles 1-arg (negate / array fold), 2-arg primary
/// (numeric or datetime), and variadic (left-fold subtractive).
#[inline]
pub(crate) fn evaluate_subtract_arena<'a>(
    args: &'a [CompiledNode],
    actx: &mut ArenaContextStack<'a>,
    engine: &DataLogic,
    arena: &'a Bump,
) -> Result<&'a ArenaValue<'a>> {
    if args.is_empty() {
        return Err(crate::constants::invalid_args());
    }
    if args.len() == 1 {
        return subtract_one_arg(&args[0], actx, engine, arena);
    }
    if args.len() == 2 {
        return subtract_two_arg(&args[0], &args[1], actx, engine, arena);
    }
    subtract_variadic(args, actx, engine, arena)
}

#[inline]
fn subtract_one_arg<'a>(
    arg: &'a CompiledNode,
    actx: &mut ArenaContextStack<'a>,
    engine: &DataLogic,
    arena: &'a Bump,
) -> Result<&'a ArenaValue<'a>> {
    let av = engine.evaluate_arena_node(arg, actx, arena)?;

    // Array fold case: (first - second - ...).
    if let ArenaValue::Array(items) = av {
        if items.is_empty() {
            return Err(crate::constants::invalid_args());
        }
        let mut result = coerce_arena_to_number_cfg(&items[0], engine)
            .ok_or_else(crate::constants::nan_error)?;
        for elem in &items[1..] {
            let n =
                coerce_arena_to_number_cfg(elem, engine).ok_or_else(crate::constants::nan_error)?;
            result -= n;
        }
        return Ok(arena_number(arena, NumberValue::from_f64(result)));
    }
    // Negate single value (preserve integer typing when possible).
    if let Some(i) = av.as_i64() {
        return Ok(arena_number(
            arena,
            i.checked_neg()
                .map(NumberValue::from_i64)
                .unwrap_or_else(|| NumberValue::from_f64(-(i as f64))),
        ));
    }
    if let Some(f) = coerce_arena_to_number_cfg(av, engine) {
        return Ok(arena_number(arena, NumberValue::from_f64(-f)));
    }
    Err(crate::constants::nan_error())
}

#[inline]
fn subtract_two_arg<'a>(
    a: &'a CompiledNode,
    b: &'a CompiledNode,
    actx: &mut ArenaContextStack<'a>,
    engine: &DataLogic,
    arena: &'a Bump,
) -> Result<&'a ArenaValue<'a>> {
    let a_av = engine.evaluate_arena_node(a, actx, arena)?;
    let b_av = engine.evaluate_arena_node(b, actx, arena)?;

    // Integer-preserving fast path.
    if let (Some(ia), Some(ib)) = (a_av.as_i64(), b_av.as_i64()) {
        return Ok(arena_number(
            arena,
            try_int_op(ia, ib, i64::checked_sub, |x, y| x - y),
        ));
    }

    if let Some((i1, i2)) = coerce_pair_int(a_av, b_av, engine) {
        return Ok(arena_number(
            arena,
            try_int_op(i1, i2, i64::checked_sub, |x, y| x - y),
        ));
    }
    if let Some((f1, f2)) = coerce_pair_f64(a_av, b_av, engine) {
        return Ok(arena_number(arena, NumberValue::from_f64(f1 - f2)));
    }

    // Datetime / duration arithmetic.
    #[cfg(feature = "datetime")]
    {
        if let Some(av) = super::datetime_arith::arena_datetime_subtract(a_av, b_av, arena) {
            return Ok(av);
        }
    }

    Err(crate::constants::nan_error())
}

/// Variadic (>2) subtract: integer fast path with overflow promotion.
#[inline]
fn subtract_variadic<'a>(
    args: &'a [CompiledNode],
    actx: &mut ArenaContextStack<'a>,
    engine: &DataLogic,
    arena: &'a Bump,
) -> Result<&'a ArenaValue<'a>> {
    let first_av = engine.evaluate_arena_node(&args[0], actx, arena)?;
    let mut all_int =
        first_av.as_i64().is_some() || try_coerce_arena_to_integer_cfg(first_av, engine).is_some();
    let mut int_acc: i64 = first_av
        .as_i64()
        .or_else(|| try_coerce_arena_to_integer_cfg(first_av, engine))
        .unwrap_or_default();
    let mut float_acc: f64 = match coerce_arena_to_number_cfg(first_av, engine) {
        Some(f) => f,
        None => return Err(crate::constants::nan_error()),
    };

    for arg in args.iter().skip(1) {
        let av = engine.evaluate_arena_node(arg, actx, arena)?;
        if all_int
            && let Some(i) = av
                .as_i64()
                .or_else(|| try_coerce_arena_to_integer_cfg(av, engine))
        {
            match int_acc.checked_sub(i) {
                Some(r) => int_acc = r,
                None => {
                    all_int = false;
                    float_acc = int_acc as f64 - i as f64;
                }
            }
            continue;
        }
        if let Some(f) = coerce_arena_to_number_cfg(av, engine) {
            if all_int {
                all_int = false;
                float_acc = int_acc as f64 - f;
            } else {
                float_acc -= f;
            }
        } else {
            match handle_nan(engine)? {
                NanAction::Skip => continue,
                NanAction::ReturnNull => return Ok(crate::arena::pool::singleton_null()),
            }
        }
    }

    if all_int {
        Ok(arena_number(arena, NumberValue::from_i64(int_acc)))
    } else {
        Ok(arena_number(arena, NumberValue::from_f64(float_acc)))
    }
}

/// 1-arg `+` / `*`: literal-array reject, then either array-fold the elements
/// or treat as a single-value sum/product.
fn arena_one_arg_arith<'a>(
    arg: &'a CompiledNode,
    actx: &mut ArenaContextStack<'a>,
    engine: &DataLogic,
    arena: &'a Bump,
    op: ArithOp,
) -> Result<&'a ArenaValue<'a>> {
    // Literal array argument is invalid for + / *. Apply NaN config (default
    // ThrowError → propagates the error up).
    let is_literal_array = matches!(arg, CompiledNode::Array { .. })
        || matches!(
            arg,
            CompiledNode::Value {
                value: Value::Array(_),
                ..
            }
        );
    if is_literal_array {
        return match handle_nan(engine)? {
            NanAction::Skip => Ok(arena_number(
                arena,
                NumberValue::from_i64(op.identity_int()),
            )),
            NanAction::ReturnNull => Ok(crate::arena::pool::singleton_null()),
        };
    }

    let av = engine.evaluate_arena_node(arg, actx, arena)?;

    // Array result (e.g. from `var "items"`): fold all elements.
    if let ArenaValue::Array(items) = av {
        return one_arg_array_fold(items, engine, arena, op);
    }

    // Non-array single value: coerce and return (op identity * coerced).
    if let Some(i) = try_coerce_arena_to_integer_cfg(av, engine) {
        return match op.combine_int(op.identity_int(), i) {
            Some(r) => Ok(arena_number(arena, NumberValue::from_i64(r))),
            None => Ok(arena_number(
                arena,
                NumberValue::from_f64(op.combine_f(op.identity_int() as f64, i as f64)),
            )),
        };
    }
    if let Some(f) = coerce_arena_to_number_cfg(av, engine) {
        return Ok(arena_number(
            arena,
            NumberValue::from_f64(op.combine_f(op.identity_int() as f64, f)),
        ));
    }
    match handle_nan(engine)? {
        NanAction::Skip => Ok(arena_number(
            arena,
            NumberValue::from_i64(op.identity_int()),
        )),
        NanAction::ReturnNull => Ok(crate::arena::pool::singleton_null()),
    }
}

/// Fold an arena-resident array under `op` (`+` or `*`) with integer fast
/// path and overflow-to-f64.
#[inline]
fn one_arg_array_fold<'a>(
    items: &[ArenaValue<'a>],
    engine: &DataLogic,
    arena: &'a Bump,
    op: ArithOp,
) -> Result<&'a ArenaValue<'a>> {
    if items.is_empty() {
        return Ok(arena_number(
            arena,
            NumberValue::from_i64(op.identity_int()),
        ));
    }
    let mut all_int = true;
    let mut int_acc: i64 = op.identity_int();
    let mut float_acc: f64 = op.identity_int() as f64;
    for item in items.iter() {
        let int_opt = try_coerce_arena_to_integer_cfg(item, engine);
        let float_opt = if int_opt.is_none() {
            coerce_arena_to_number_cfg(item, engine)
        } else {
            None
        };
        if let Some(iv) = int_opt {
            if all_int {
                match op.combine_int(int_acc, iv) {
                    Some(r) => int_acc = r,
                    None => {
                        all_int = false;
                        float_acc = op.combine_f(int_acc as f64, iv as f64);
                    }
                }
            } else {
                float_acc = op.combine_f(float_acc, iv as f64);
            }
        } else if let Some(fv) = float_opt {
            if all_int {
                all_int = false;
                float_acc = op.combine_f(int_acc as f64, fv);
            } else {
                float_acc = op.combine_f(float_acc, fv);
            }
        } else {
            match handle_nan(engine)? {
                NanAction::Skip => continue,
                NanAction::ReturnNull => return Ok(crate::arena::pool::singleton_null()),
            }
        }
    }
    if all_int {
        Ok(arena_number(arena, NumberValue::from_i64(int_acc)))
    } else {
        Ok(arena_number(arena, NumberValue::from_f64(float_acc)))
    }
}
