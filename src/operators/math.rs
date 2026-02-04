//! Unary math operator implementations (abs, ceil, floor).
//!
//! Provides operators for absolute value, ceiling, and floor rounding.
//! All support both single values and variadic calls returning arrays.

use serde_json::Value;

use super::helpers::get_number_strict;
use crate::constants::INVALID_ARGS;
use crate::{CompiledNode, ContextStack, DataLogic, Error, Result};

/// Whether the result of a unary math op should always be cast to integer.
#[derive(Clone, Copy)]
enum IntegerPreservation {
    /// Always cast result to i64 (ceil, floor — result is always whole)
    AlwaysInteger,
    /// Only cast to i64 if the result is a whole number (abs)
    IfWhole,
}

/// Converts an f64 result to a Value according to the preservation mode.
#[inline]
fn to_value(val: f64, mode: IntegerPreservation) -> Value {
    match mode {
        IntegerPreservation::AlwaysInteger => Value::Number((val as i64).into()),
        IntegerPreservation::IfWhole => {
            let int_val = val as i64;
            if int_val as f64 == val {
                Value::Number(int_val.into())
            } else {
                serde_json::Number::from_f64(val)
                    .map(Value::Number)
                    .unwrap_or(Value::Null)
            }
        }
    }
}

/// Generic unary math operator: evaluates args, applies `op_fn` to each number.
#[inline]
fn evaluate_unary_math(
    args: &[CompiledNode],
    context: &mut ContextStack,
    engine: &DataLogic,
    op_fn: fn(f64) -> f64,
    mode: IntegerPreservation,
) -> Result<Value> {
    if args.is_empty() {
        return Err(Error::InvalidArguments(INVALID_ARGS.into()));
    }

    if args.len() > 1 {
        let mut results = Vec::new();
        for arg in args {
            let value = engine.evaluate_node_cow(arg, context)?;
            if let Some(num) = get_number_strict(&value) {
                results.push(to_value(op_fn(num), mode));
            } else {
                return Err(Error::InvalidArguments(INVALID_ARGS.into()));
            }
        }
        return Ok(Value::Array(results));
    }

    let value = engine.evaluate_node_cow(&args[0], context)?;

    if let Some(num) = get_number_strict(&value) {
        Ok(to_value(op_fn(num), mode))
    } else {
        Err(Error::InvalidArguments(INVALID_ARGS.into()))
    }
}

/// Absolute value operator function (abs)
#[inline]
pub fn evaluate_abs(
    args: &[CompiledNode],
    context: &mut ContextStack,
    engine: &DataLogic,
) -> Result<Value> {
    evaluate_unary_math(
        args,
        context,
        engine,
        f64::abs,
        IntegerPreservation::IfWhole,
    )
}

/// Ceiling operator function (ceil)
#[inline]
pub fn evaluate_ceil(
    args: &[CompiledNode],
    context: &mut ContextStack,
    engine: &DataLogic,
) -> Result<Value> {
    evaluate_unary_math(
        args,
        context,
        engine,
        f64::ceil,
        IntegerPreservation::AlwaysInteger,
    )
}

/// Floor operator function (floor)
#[inline]
pub fn evaluate_floor(
    args: &[CompiledNode],
    context: &mut ContextStack,
    engine: &DataLogic,
) -> Result<Value> {
    evaluate_unary_math(
        args,
        context,
        engine,
        f64::floor,
        IntegerPreservation::AlwaysInteger,
    )
}
