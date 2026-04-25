//! Arithmetic operators for numeric computations.
//!
//! This module provides all arithmetic operators with support for:
//! - Integer and floating-point arithmetic
//! - Overflow protection with automatic promotion to float
//! - DateTime and Duration arithmetic
//! - Configurable NaN handling
//!
//! # Operators
//!
//! | Operator | Description | Example |
//! |----------|-------------|---------|
//! | `+` | Addition | `{"+": [1, 2, 3]}` → `6` |
//! | `-` | Subtraction | `{"-": [10, 3]}` → `7` |
//! | `*` | Multiplication | `{"*": [2, 3, 4]}` → `24` |
//! | `/` | Division | `{"/": [10, 2]}` → `5` |
//! | `%` | Modulo | `{"%": [10, 3]}` → `1` |
//! | `min` | Minimum value | `{"min": [3, 1, 4]}` → `1` |
//! | `max` | Maximum value | `{"max": [3, 1, 4]}` → `4` |
//!
//! # Overflow Handling Pattern
//!
//! All arithmetic operators use the same pattern for overflow protection:
//!
//! 1. **Track integer precision**: Use `all_integers` flag to track if we can stay in i64
//! 2. **Checked arithmetic**: Use `checked_add`, `checked_mul`, etc. for i64 operations
//! 3. **Overflow promotion**: On overflow, switch to f64 and continue accumulating
//! 4. **Result preservation**: Return i64 when possible, f64 otherwise
//!
//! This approach maximizes integer precision while gracefully handling overflow:
//!
//! ```text
//! // Example overflow handling in addition:
//! match int_sum.checked_add(i) {
//!     Some(sum) => int_sum = sum,         // No overflow: continue with integers
//!     None => {
//!         all_integers = false;            // Overflow: switch to float
//!         float_sum = int_sum as f64 + i as f64;
//!     }
//! }
//! ```
//!
//! # DateTime Arithmetic
//!
//! Arithmetic operators also handle DateTime and Duration values:
//! - `datetime + duration` → `datetime`
//! - `datetime - datetime` → `duration`
//! - `duration + duration` → `duration`
//! - `duration * number` → `duration`
//!
//! # NaN Handling
//!
//! When a value cannot be coerced to a number, behavior depends on `NanHandling` config:
//! - `ThrowError`: Return error (default)
//! - `IgnoreValue`: Skip non-numeric values
//! - `CoerceToZero`: Treat as 0
//! - `ReturnNull`: Return null

use serde_json::Value;

use super::helpers::{
    create_number_value, safe_add, safe_divide, safe_modulo, safe_multiply, safe_subtract,
};
#[cfg(feature = "datetime")]
use super::helpers::{extract_datetime_value, extract_duration_value};
use crate::config::NanHandling;
use crate::constants::INVALID_ARGS;
use crate::value_helpers::{coerce_to_number, try_coerce_to_integer};
use crate::{CompiledNode, ContextStack, DataLogic, Error, Result};

/// Result of NaN handling check: what the caller should do with a non-numeric value.
enum NanAction {
    /// Skip/ignore this value (IgnoreValue or CoerceToZero)
    Skip,
    /// Return null immediately
    ReturnNull,
}

/// Checks the engine's NaN handling config and returns the appropriate action.
/// Returns `Err` for ThrowError, `Ok(NanAction)` otherwise.
#[inline]
fn handle_nan(engine: &DataLogic) -> Result<NanAction> {
    match engine.config().arithmetic_nan_handling {
        NanHandling::ThrowError => Err(crate::constants::nan_error()),
        NanHandling::IgnoreValue | NanHandling::CoerceToZero => Ok(NanAction::Skip),
        NanHandling::ReturnNull => Ok(NanAction::ReturnNull),
    }
}

/// Helper to convert float to integer if it's a whole number
#[inline]
fn number_value(f: f64) -> Value {
    if f.is_finite() && f.floor() == f && f >= i64::MIN as f64 && f <= i64::MAX as f64 {
        Value::Number((f as i64).into())
    } else {
        create_number_value(f)
    }
}

/// Addition operator function (+) - variadic
#[inline]
pub fn evaluate_add(
    args: &[CompiledNode],
    context: &mut ContextStack,
    engine: &DataLogic,
) -> Result<Value> {
    if args.is_empty() {
        return Ok(Value::Number(0.into()));
    }

    // Special case: single array argument - sum all elements
    if args.len() == 1 {
        // Check if the argument is a literal array (which is invalid for addition)
        if matches!(&args[0], CompiledNode::Array { .. }) {
            // Literal array as argument - this is invalid for addition
            return Err(crate::constants::nan_error());
        }

        // Also check if it's a Value node containing an array (from compilation)
        if let CompiledNode::Value { value, .. } = &args[0]
            && matches!(value, Value::Array(_))
        {
            return Err(crate::constants::nan_error());
        }

        let value = engine.evaluate_node(&args[0], context)?;
        if let Value::Array(arr) = value {
            // Array from operator evaluation - sum the elements
            if arr.is_empty() {
                return Ok(Value::Number(0.into())); // Identity element for addition
            }

            // Fast path: every element is Number(i64). Avoids per-elem engine
            // config checks in try_coerce_to_integer.
            let mut int_sum: i64 = 0;
            let mut fast_ok = true;
            for elem in &arr {
                if let Value::Number(n) = elem
                    && let Some(i) = n.as_i64()
                {
                    match int_sum.checked_add(i) {
                        Some(s) => int_sum = s,
                        None => {
                            fast_ok = false;
                            break;
                        }
                    }
                } else {
                    fast_ok = false;
                    break;
                }
            }
            if fast_ok {
                return Ok(Value::Number(int_sum.into()));
            }

            // Don't recursively call evaluate - that would treat the array as literal
            // Instead, evaluate each element and sum them
            let mut all_integers = true;
            let mut int_sum: i64 = 0;
            let mut float_sum = 0.0;

            for elem in &arr {
                // Array elements are already evaluated values
                if let Some(i) = try_coerce_to_integer(elem, engine) {
                    if all_integers {
                        // Check for overflow before adding
                        match int_sum.checked_add(i) {
                            Some(sum) => int_sum = sum,
                            None => {
                                // Overflow detected, switch to float
                                all_integers = false;
                                float_sum = int_sum as f64 + i as f64;
                            }
                        }
                    } else {
                        float_sum = safe_add(float_sum, i as f64);
                    }
                } else if let Some(f) = coerce_to_number(elem, engine) {
                    all_integers = false;
                    float_sum = safe_add(float_sum, f);
                } else {
                    match handle_nan(engine)? {
                        NanAction::Skip => continue,
                        NanAction::ReturnNull => return Ok(Value::Null),
                    }
                }
            }

            return if all_integers {
                Ok(Value::Number(int_sum.into()))
            } else {
                Ok(number_value(float_sum))
            };
        }
    }

    // Special case for two arguments (most common)
    if args.len() == 2 {
        let first = engine.evaluate_node_cow(&args[0], context)?;
        let second = engine.evaluate_node_cow(&args[1], context)?;

        // Fast path: both are numbers (most common case) — skip datetime checks
        if let (Some(i1), Some(i2)) = (
            try_coerce_to_integer(&first, engine),
            try_coerce_to_integer(&second, engine),
        ) {
            return match i1.checked_add(i2) {
                Some(sum) => Ok(Value::Number(sum.into())),
                None => Ok(number_value(i1 as f64 + i2 as f64)),
            };
        }
        if let (Some(f1), Some(f2)) = (
            coerce_to_number(&first, engine),
            coerce_to_number(&second, engine),
        ) {
            return Ok(number_value(safe_add(f1, f2)));
        }

        // Slow path: datetime/duration arithmetic
        #[cfg(feature = "datetime")]
        {
            // Parse first: try datetime, then duration (mutually exclusive)
            let first_dt = extract_datetime_value(first.as_ref());
            let first_dur = if first_dt.is_none() {
                extract_duration_value(first.as_ref())
            } else {
                None
            };

            // For addition, second is only needed as duration
            let second_dur = extract_duration_value(second.as_ref());

            // DateTime + Duration
            if let (Some(dt), Some(dur)) = (&first_dt, &second_dur) {
                let result = dt.add_duration(dur);
                return Ok(Value::String(result.to_iso_string()));
            }

            // Duration + Duration
            if let (Some(dur1), Some(dur2)) = (&first_dur, &second_dur) {
                let result = dur1.add(dur2);
                return Ok(Value::String(result.to_string()));
            }
        }

        // Non-numeric, non-datetime values — handle NaN per config
        // At least one of the two values is not coercible to number
        let mut sum = 0.0f64;
        for val in [first.as_ref(), second.as_ref()] {
            if let Some(f) = coerce_to_number(val, engine) {
                sum = safe_add(sum, f);
            } else {
                match handle_nan(engine)? {
                    NanAction::Skip => {}
                    NanAction::ReturnNull => return Ok(Value::Null),
                }
            }
        }
        return Ok(number_value(sum));
    }

    // Regular numeric addition
    // Check if all values are integers
    let mut all_integers = true;
    let mut int_sum: i64 = 0;
    let mut float_sum = 0.0;

    for arg in args {
        // Check if this argument is a literal array (invalid for addition)
        if matches!(arg, CompiledNode::Array { .. }) {
            match handle_nan(engine)? {
                NanAction::Skip => continue,
                NanAction::ReturnNull => return Ok(Value::Null),
            }
        }

        let value = engine.evaluate_node_cow(arg, context)?;

        // Arrays and objects are invalid for addition
        if matches!(value.as_ref(), Value::Array(_) | Value::Object(_)) {
            match handle_nan(engine)? {
                NanAction::Skip => continue,
                NanAction::ReturnNull => return Ok(Value::Null),
            }
        }

        // Try integer coercion first
        if let Some(i) = try_coerce_to_integer(&value, engine) {
            if all_integers {
                // Check for overflow before adding
                match int_sum.checked_add(i) {
                    Some(sum) => int_sum = sum,
                    None => {
                        // Overflow detected, switch to float
                        all_integers = false;
                        float_sum = int_sum as f64 + i as f64;
                    }
                }
            } else {
                float_sum = safe_add(float_sum, i as f64);
            }
        } else if let Some(f) = coerce_to_number(&value, engine) {
            // Switch from integer to float mode
            if all_integers {
                all_integers = false;
                float_sum = int_sum as f64 + f;
            } else {
                float_sum = safe_add(float_sum, f);
            }
        } else {
            match handle_nan(engine)? {
                NanAction::Skip => continue,
                NanAction::ReturnNull => return Ok(Value::Null),
            }
        }
    }

    // Return integer if all inputs were integers, otherwise float
    if all_integers {
        Ok(Value::Number(int_sum.into()))
    } else {
        Ok(number_value(float_sum))
    }
}

/// Subtraction operator function (-) - also handles negation
#[inline]
pub fn evaluate_subtract(
    args: &[CompiledNode],
    context: &mut ContextStack,
    engine: &DataLogic,
) -> Result<Value> {
    if args.is_empty() {
        return Err(Error::InvalidArguments(INVALID_ARGS.into()));
    }

    let first = engine.evaluate_node(&args[0], context)?;

    if args.len() == 1 {
        // Check if it's an array - subtract all elements
        if let Value::Array(arr) = first {
            if arr.is_empty() {
                return Err(Error::InvalidArguments(INVALID_ARGS.into()));
            }
            // Subtract elements: first - second - third - ...
            let mut result =
                coerce_to_number(&arr[0], engine).ok_or_else(crate::constants::nan_error)?;

            for elem in &arr[1..] {
                let num = coerce_to_number(elem, engine).ok_or_else(crate::constants::nan_error)?;
                result = safe_subtract(result, num);
            }

            return Ok(number_value(result));
        }

        // Negation
        if let Value::Number(n) = &first {
            if let Some(i) = n.as_i64() {
                return Ok(Value::Number((-i).into()));
            } else if let Some(f) = n.as_f64() {
                return Ok(number_value(-f));
            }
        }
        let first_num = coerce_to_number(&first, engine).ok_or_else(crate::constants::nan_error)?;
        Ok(number_value(-first_num))
    } else if args.len() == 2 {
        let second = engine.evaluate_node_cow(&args[1], context)?;

        // Fast path: both are numbers (most common case) — skip datetime checks
        if let (Some(i1), Some(i2)) = (
            try_coerce_to_integer(&first, engine),
            try_coerce_to_integer(&second, engine),
        ) {
            return match i1.checked_sub(i2) {
                Some(diff) => Ok(Value::Number(diff.into())),
                None => Ok(number_value(i1 as f64 - i2 as f64)),
            };
        }
        if let (Some(f1), Some(f2)) = (
            coerce_to_number(&first, engine),
            coerce_to_number(&second, engine),
        ) {
            return Ok(number_value(safe_subtract(f1, f2)));
        }

        // Slow path: datetime/duration arithmetic
        #[cfg(feature = "datetime")]
        {
            // Parse first: try datetime, then duration (mutually exclusive)
            let first_dt = extract_datetime_value(&first);
            let first_dur = if first_dt.is_none() {
                extract_duration_value(&first)
            } else {
                None
            };

            // Parse second: try datetime, then duration (mutually exclusive)
            let second_dt = extract_datetime_value(second.as_ref());
            let second_dur = if second_dt.is_none() {
                extract_duration_value(second.as_ref())
            } else {
                None
            };

            // DateTime - DateTime = Duration (check this first)
            if let (Some(dt1), Some(dt2)) = (&first_dt, &second_dt) {
                let result = dt1.diff(dt2);
                return Ok(Value::String(result.to_string()));
            }

            // DateTime - Duration
            if let (Some(dt), Some(dur)) = (&first_dt, &second_dur) {
                let result = dt.sub_duration(dur);
                return Ok(Value::String(result.to_iso_string()));
            }

            // Duration - Duration
            if let (Some(dur1), Some(dur2)) = (&first_dur, &second_dur) {
                let result = dur1.sub(dur2);
                return Ok(Value::String(result.to_string()));
            }
        }

        // Try integer coercion first for both operands
        if let (Some(i1), Some(i2)) = (
            try_coerce_to_integer(&first, engine),
            try_coerce_to_integer(&second, engine),
        ) {
            // Check for overflow in subtraction
            match i1.checked_sub(i2) {
                Some(result) => return Ok(Value::Number(result.into())),
                None => {
                    // Overflow, fall through to float calculation
                }
            }
        }

        let first_num = coerce_to_number(&first, engine).ok_or_else(crate::constants::nan_error)?;
        let second_num =
            coerce_to_number(&second, engine).ok_or_else(crate::constants::nan_error)?;

        Ok(number_value(first_num - second_num))
    } else {
        // Variadic subtraction (3+ arguments)
        // Check if all values are integers
        let mut all_integers = true;
        let mut int_result = if let Some(i) = try_coerce_to_integer(&first, engine) {
            i
        } else {
            all_integers = false;
            0
        };
        let mut float_result = if let Some(f) = coerce_to_number(&first, engine) {
            f
        } else {
            return Ok(Value::Null);
        };

        // Subtract remaining arguments
        for item in args.iter().skip(1) {
            let value = engine.evaluate_node_cow(item, context)?;

            if all_integers {
                if let Some(i) = try_coerce_to_integer(&value, engine) {
                    // Check for overflow in subtraction
                    match int_result.checked_sub(i) {
                        Some(result) => int_result = result,
                        None => {
                            // Overflow detected, switch to float
                            all_integers = false;
                            float_result = int_result as f64 - i as f64;
                        }
                    }
                } else if let Some(f) = coerce_to_number(&value, engine) {
                    all_integers = false;
                    float_result = int_result as f64 - f;
                } else {
                    match handle_nan(engine)? {
                        NanAction::Skip => continue,
                        NanAction::ReturnNull => return Ok(Value::Null),
                    }
                }
            } else if let Some(f) = coerce_to_number(&value, engine) {
                float_result = safe_subtract(float_result, f);
            } else {
                match handle_nan(engine)? {
                    NanAction::Skip => continue,
                    NanAction::ReturnNull => return Ok(Value::Null),
                }
            }
        }

        if all_integers {
            Ok(Value::Number(int_result.into()))
        } else {
            Ok(number_value(float_result))
        }
    }
}

/// Multiplication operator function (*) - variadic
#[inline]
pub fn evaluate_multiply(
    args: &[CompiledNode],
    context: &mut ContextStack,
    engine: &DataLogic,
) -> Result<Value> {
    if args.is_empty() {
        return Ok(Value::Number(1.into()));
    }

    // Special case: single array argument - multiply all elements
    if args.len() == 1 {
        let value = engine.evaluate_node(&args[0], context)?;
        if let Value::Array(arr) = value {
            // Array from operator evaluation - multiply the elements
            if arr.is_empty() {
                return Ok(Value::Number(1.into())); // Identity element for multiplication
            }

            // Fast path: every element is Number(i64).
            let mut int_product: i64 = 1;
            let mut fast_ok = true;
            for elem in &arr {
                if let Value::Number(n) = elem
                    && let Some(i) = n.as_i64()
                {
                    match int_product.checked_mul(i) {
                        Some(p) => int_product = p,
                        None => {
                            fast_ok = false;
                            break;
                        }
                    }
                } else {
                    fast_ok = false;
                    break;
                }
            }
            if fast_ok {
                return Ok(Value::Number(int_product.into()));
            }

            // Don't recursively call evaluate - that would treat the array as literal
            // Instead, evaluate each element and multiply them
            let mut all_integers = true;
            let mut int_product: i64 = 1;
            let mut float_product = 1.0;

            for elem in &arr {
                // Array elements are already evaluated values
                if let Some(i) = try_coerce_to_integer(elem, engine) {
                    if all_integers {
                        match int_product.checked_mul(i) {
                            Some(p) => int_product = p,
                            None => {
                                all_integers = false;
                                float_product = int_product as f64 * i as f64;
                            }
                        }
                    } else {
                        float_product = safe_multiply(float_product, i as f64);
                    }
                } else if let Some(f) = coerce_to_number(elem, engine) {
                    if all_integers {
                        float_product = int_product as f64 * f;
                    } else {
                        float_product = safe_multiply(float_product, f);
                    }
                    all_integers = false;
                } else {
                    match handle_nan(engine)? {
                        NanAction::Skip => continue,
                        NanAction::ReturnNull => return Ok(Value::Null),
                    }
                }
            }

            return if all_integers {
                Ok(Value::Number(int_product.into()))
            } else {
                Ok(number_value(float_product))
            };
        }
    }

    // Special case for two arguments
    if args.len() == 2 {
        let first = engine.evaluate_node_cow(&args[0], context)?;
        let second = engine.evaluate_node_cow(&args[1], context)?;

        // Fast path: both are numbers (most common case) — skip duration checks
        if let (Some(i1), Some(i2)) = (
            try_coerce_to_integer(&first, engine),
            try_coerce_to_integer(&second, engine),
        ) {
            return match i1.checked_mul(i2) {
                Some(product) => Ok(Value::Number(product.into())),
                None => Ok(number_value(i1 as f64 * i2 as f64)),
            };
        }
        if let (Some(f1), Some(f2)) = (
            coerce_to_number(&first, engine),
            coerce_to_number(&second, engine),
        ) {
            return Ok(number_value(safe_multiply(f1, f2)));
        }

        // Slow path: duration * number or number * duration
        #[cfg(feature = "datetime")]
        {
            let first_dur = extract_duration_value(first.as_ref());

            if let Some(dur) = &first_dur
                && let Some(factor) = coerce_to_number(&second, engine)
            {
                let result = dur.multiply(factor);
                return Ok(Value::String(result.to_string()));
            }

            // Number * Duration (only if first wasn't a duration)
            if first_dur.is_none() {
                let second_dur = extract_duration_value(second.as_ref());

                if let Some(dur) = second_dur
                    && let Some(factor) = coerce_to_number(&first, engine)
                {
                    let result = dur.multiply(factor);
                    return Ok(Value::String(result.to_string()));
                }
            }
        }
    }

    // Regular numeric multiplication
    // Check if all values are integers
    let mut all_integers = true;
    let mut int_product: i64 = 1;
    let mut float_product = 1.0;

    for arg in args {
        let value = engine.evaluate_node_cow(arg, context)?;

        // Try integer coercion first
        if let Some(i) = try_coerce_to_integer(&value, engine) {
            if all_integers {
                match int_product.checked_mul(i) {
                    Some(p) => int_product = p,
                    None => {
                        all_integers = false;
                        float_product = int_product as f64 * i as f64;
                    }
                }
            } else {
                float_product = safe_multiply(float_product, i as f64);
            }
        } else if let Some(f) = coerce_to_number(&value, engine) {
            if all_integers {
                float_product = int_product as f64 * f;
            } else {
                float_product = safe_multiply(float_product, f);
            }
            all_integers = false;
        } else {
            match handle_nan(engine)? {
                NanAction::Skip => {}
                NanAction::ReturnNull => return Ok(Value::Null),
            }
        }
    }

    if all_integers {
        Ok(Value::Number(int_product.into()))
    } else {
        Ok(number_value(float_product))
    }
}

/// Division operator function (/)
#[inline]
pub fn evaluate_divide(
    args: &[CompiledNode],
    context: &mut ContextStack,
    engine: &DataLogic,
) -> Result<Value> {
    if args.is_empty() {
        return Err(Error::InvalidArguments(INVALID_ARGS.into()));
    }

    // Special case: single argument
    if args.len() == 1 {
        let value = engine.evaluate_node(&args[0], context)?;

        // If it's an array, divide all elements sequentially
        if let Value::Array(arr) = value {
            if arr.is_empty() {
                return Err(Error::InvalidArguments(INVALID_ARGS.into()));
            }
            // Divide elements: first / second / third / ...
            let mut result =
                coerce_to_number(&arr[0], engine).ok_or_else(crate::constants::nan_error)?;

            for elem in &arr[1..] {
                let num = coerce_to_number(elem, engine).ok_or_else(crate::constants::nan_error)?;
                if num == 0.0 {
                    return Err(crate::constants::nan_error());
                }
                result = safe_divide(result, num);
            }

            return Ok(number_value(result));
        }

        // Single non-array argument: 1 / value
        let num = coerce_to_number(&value, engine).ok_or_else(crate::constants::nan_error)?;

        if num == 0.0 {
            return Err(crate::constants::nan_error());
        }

        // Try to preserve integer type with overflow check
        if let Some(i) = try_coerce_to_integer(&value, engine)
            && i != 0
        {
            // Special case: avoid overflow when dividing by -1
            if i == -1 {
                return Ok(Value::Number((-1).into()));
            }
            if 1 % i == 0 {
                return Ok(Value::Number((1 / i).into()));
            }
        }

        return Ok(number_value(1.0 / num));
    }

    let first = engine.evaluate_node(&args[0], context)?;

    if args.len() == 2 {
        let second = engine.evaluate_node_cow(&args[1], context)?;

        // Duration / Number
        #[cfg(feature = "datetime")]
        {
            let first_dur = extract_duration_value(&first);

            if let Some(dur) = first_dur
                && let Some(divisor) = coerce_to_number(&second, engine)
            {
                if divisor == 0.0 {
                    return Err(crate::constants::nan_error());
                }
                let result = dur.divide(divisor);
                return Ok(Value::String(result.to_string()));
            }
        }

        // Try integer division first if both can be coerced to integers
        if let (Some(i1), Some(i2)) = (
            try_coerce_to_integer(&first, engine),
            try_coerce_to_integer(&second, engine),
        ) {
            if i2 == 0 {
                return Err(crate::constants::nan_error());
            }
            // Special case: avoid overflow when dividing MIN by -1
            if i1 == i64::MIN && i2 == -1 {
                // This would overflow, use float instead
                return Ok(number_value(-(i64::MIN as f64)));
            }
            // Check if division is exact (no remainder)
            if i1 % i2 == 0 {
                return Ok(Value::Number((i1 / i2).into()));
            }
        }

        let first_num = coerce_to_number(&first, engine).ok_or_else(crate::constants::nan_error)?;
        let second_num =
            coerce_to_number(&second, engine).ok_or_else(crate::constants::nan_error)?;

        if second_num == 0.0 {
            return Err(crate::constants::nan_error());
        }

        Ok(number_value(first_num / second_num))
    } else {
        // Variadic division (3+ arguments)
        // Try to maintain integer type if possible
        let mut all_integers = true;
        let mut int_result = if let Some(i) = try_coerce_to_integer(&first, engine) {
            i
        } else {
            all_integers = false;
            0
        };
        let mut float_result =
            coerce_to_number(&first, engine).ok_or_else(crate::constants::nan_error)?;

        for item in args.iter().skip(1) {
            let value = engine.evaluate_node_cow(item, context)?;

            if all_integers {
                if let Some(divisor) = try_coerce_to_integer(&value, engine) {
                    if divisor == 0 {
                        return Err(crate::constants::nan_error());
                    }
                    // Special case: avoid overflow when dividing MIN by -1
                    if int_result == i64::MIN && divisor == -1 {
                        all_integers = false;
                        float_result = -(i64::MIN as f64);
                    } else if int_result % divisor == 0 {
                        // Check if division is exact
                        int_result /= divisor;
                    } else {
                        // Switch to float
                        all_integers = false;
                        float_result = int_result as f64 / divisor as f64;
                    }
                } else if let Some(divisor) = coerce_to_number(&value, engine) {
                    if divisor == 0.0 {
                        return Err(crate::constants::nan_error());
                    }
                    all_integers = false;
                    float_result = int_result as f64 / divisor;
                } else {
                    return Ok(Value::Null);
                }
            } else {
                let divisor =
                    coerce_to_number(&value, engine).ok_or_else(crate::constants::nan_error)?;
                if divisor == 0.0 {
                    return Err(crate::constants::nan_error());
                }
                float_result = safe_divide(float_result, divisor);
            }
        }

        if all_integers {
            Ok(Value::Number(int_result.into()))
        } else {
            Ok(number_value(float_result))
        }
    }
}

/// Modulo operator function (%)
#[inline]
pub fn evaluate_modulo(
    args: &[CompiledNode],
    context: &mut ContextStack,
    engine: &DataLogic,
) -> Result<Value> {
    if args.is_empty() {
        return Err(Error::InvalidArguments(INVALID_ARGS.into()));
    }

    // Special case: single array argument - modulo all elements sequentially
    if args.len() == 1 {
        let value = engine.evaluate_node(&args[0], context)?;
        if let Value::Array(arr) = value {
            if arr.is_empty() || arr.len() < 2 {
                return Err(Error::InvalidArguments(INVALID_ARGS.into()));
            }
            // Modulo elements: first % second % third % ...
            let mut result =
                coerce_to_number(&arr[0], engine).ok_or_else(crate::constants::nan_error)?;

            for elem in &arr[1..] {
                let num = coerce_to_number(elem, engine).ok_or_else(crate::constants::nan_error)?;
                if num == 0.0 {
                    return Err(crate::constants::nan_error());
                }
                result = safe_modulo(result, num);
            }

            return Ok(number_value(result));
        }
        return Err(Error::InvalidArguments(INVALID_ARGS.into()));
    }

    if args.len() < 2 {
        return Err(Error::InvalidArguments(INVALID_ARGS.into()));
    }

    let first = engine.evaluate_node(&args[0], context)?;

    if args.len() == 2 {
        let second = engine.evaluate_node_cow(&args[1], context)?;

        // Check if both are integers
        if let (Value::Number(n1), Value::Number(n2)) = (&first, second.as_ref())
            && let (Some(i1), Some(i2)) = (n1.as_i64(), n2.as_i64())
        {
            if i2 == 0 {
                return Err(crate::constants::nan_error());
            }
            // Special case: i64::MIN % -1 would overflow in some contexts
            if i1 == i64::MIN && i2 == -1 {
                return Ok(Value::Number(0.into()));
            }
            return Ok(Value::Number((i1 % i2).into()));
        }

        let first_num = coerce_to_number(&first, engine).ok_or_else(crate::constants::nan_error)?;
        let second_num =
            coerce_to_number(&second, engine).ok_or_else(crate::constants::nan_error)?;

        if second_num == 0.0 {
            return Err(crate::constants::nan_error());
        }

        Ok(number_value(first_num % second_num))
    } else {
        // Variadic modulo (3+ arguments)
        let mut result =
            coerce_to_number(&first, engine).ok_or_else(crate::constants::nan_error)?;

        for item in args.iter().skip(1) {
            let value = engine.evaluate_node_cow(item, context)?;
            let num = coerce_to_number(&value, engine).ok_or_else(crate::constants::nan_error)?;

            if num == 0.0 {
                return Err(crate::constants::nan_error());
            }

            result = safe_modulo(result, num);
        }

        Ok(number_value(result))
    }
}

/// Max operator function - variadic
#[inline]
pub fn evaluate_max(
    args: &[CompiledNode],
    context: &mut ContextStack,
    engine: &DataLogic,
) -> Result<Value> {
    if args.is_empty() {
        return Err(Error::InvalidArguments(INVALID_ARGS.into()));
    }

    // Special case: single argument
    if args.len() == 1 {
        // Check if it's a literal array (invalid for max)
        if matches!(&args[0], CompiledNode::Array { .. }) {
            return Err(Error::InvalidArguments(INVALID_ARGS.into()));
        }
        // Also check if it's a Value node containing an array
        if let CompiledNode::Value { value, .. } = &args[0]
            && matches!(value, Value::Array(_))
        {
            return Err(Error::InvalidArguments(INVALID_ARGS.into()));
        }

        let value = engine.evaluate_node(&args[0], context)?;

        // If evaluation produced an array, find max of its elements
        if let Value::Array(arr) = value {
            if arr.is_empty() {
                return Err(Error::InvalidArguments(INVALID_ARGS.into()));
            }

            // Process array elements directly instead of recursing
            let mut max_value: Option<Value> = None;
            let mut max_num = f64::NEG_INFINITY;

            for elem in arr {
                if let Value::Number(n) = &elem {
                    if let Some(f) = n.as_f64()
                        && f > max_num
                    {
                        max_num = f;
                        max_value = Some(elem);
                    }
                } else {
                    return Err(Error::InvalidArguments(INVALID_ARGS.into()));
                }
            }

            return Ok(max_value.unwrap_or(Value::Null));
        }

        // Single non-array argument - check if it's numeric
        if !matches!(value, Value::Number(_)) {
            return Err(Error::InvalidArguments(INVALID_ARGS.into()));
        }
        return Ok(value);
    }

    let mut max_value: Option<Value> = None;
    let mut max_num = f64::NEG_INFINITY;

    for arg in args {
        let value = engine.evaluate_node(arg, context)?;

        if let Value::Number(n) = &value {
            if let Some(f) = n.as_f64()
                && f > max_num
            {
                max_num = f;
                max_value = Some(value);
            }
        } else {
            return Err(Error::InvalidArguments(INVALID_ARGS.into()));
        }
    }

    // Return the actual value that was max (preserving integer type)
    Ok(max_value.unwrap_or(Value::Null))
}

/// Min operator function - variadic
#[inline]
pub fn evaluate_min(
    args: &[CompiledNode],
    context: &mut ContextStack,
    engine: &DataLogic,
) -> Result<Value> {
    if args.is_empty() {
        return Err(Error::InvalidArguments(INVALID_ARGS.into()));
    }

    // Special case: single argument
    if args.len() == 1 {
        // Check if it's a literal array (invalid for min)
        if matches!(&args[0], CompiledNode::Array { .. }) {
            return Err(Error::InvalidArguments(INVALID_ARGS.into()));
        }
        // Also check if it's a Value node containing an array
        if let CompiledNode::Value { value, .. } = &args[0]
            && matches!(value, Value::Array(_))
        {
            return Err(Error::InvalidArguments(INVALID_ARGS.into()));
        }

        let value = engine.evaluate_node(&args[0], context)?;

        // If evaluation produced an array, find min of its elements
        if let Value::Array(arr) = value {
            if arr.is_empty() {
                return Err(Error::InvalidArguments(INVALID_ARGS.into()));
            }

            // Process array elements directly instead of recursing
            let mut min_value: Option<Value> = None;
            let mut min_num = f64::INFINITY;

            for elem in arr {
                if let Value::Number(n) = &elem {
                    if let Some(f) = n.as_f64()
                        && f < min_num
                    {
                        min_num = f;
                        min_value = Some(elem);
                    }
                } else {
                    return Err(Error::InvalidArguments(INVALID_ARGS.into()));
                }
            }

            return Ok(min_value.unwrap_or(Value::Null));
        }

        // Single non-array argument - check if it's numeric
        if !matches!(value, Value::Number(_)) {
            return Err(Error::InvalidArguments(INVALID_ARGS.into()));
        }
        return Ok(value);
    }

    let mut min_value: Option<Value> = None;
    let mut min_num = f64::INFINITY;

    for arg in args {
        let value = engine.evaluate_node(arg, context)?;

        if let Value::Number(n) = &value {
            if let Some(f) = n.as_f64()
                && f < min_num
            {
                min_num = f;
                min_value = Some(value);
            }
        } else {
            return Err(Error::InvalidArguments(INVALID_ARGS.into()));
        }
    }

    // Return the actual value that was min (preserving integer type)
    Ok(min_value.unwrap_or(Value::Null))
}

// =============================================================================
// Arena-mode array-consumer ops (Phase 5: max / min / + / *)
//
// These are "pipeline tops" — they consume an array (typically produced by an
// upstream filter/map) and return a single Number. They benefit from arena
// dispatch in two ways:
//   1. Input borrow: when args[0] is a root var, no clone of the input array.
//   2. Composition: when args[0] is filter/map/all/some/none, the arena
//      intermediate slice is consumed directly without value-mode bridging.
//
// Each op handles the SINGLE-ARG ARRAY form (e.g. `max(items)` over an array).
// The multi-arg form (`max(a, b, c)`) stays on the value path — it doesn't
// involve array iteration so arena gives no win.
// =============================================================================

use crate::arena::{ArenaValue, value_to_arena};
use crate::operators::array::{ResolvedInput, resolve_iter_input};
use bumpalo::Bump;

/// Generic helper for max/min over an arena-iterable input. `pick_better`
/// returns true when `candidate_f` should replace `best_f` (strictly better).
#[inline]
#[allow(clippy::too_many_arguments)] // 5 contextual + init/pick_better/op_name
fn arena_min_max<'a>(
    args: &[CompiledNode],
    context: &mut ContextStack,
    engine: &DataLogic,
    arena: &'a Bump,
    root: &'a Value,
    init: f64,
    pick_better: fn(f64, f64) -> bool,
    op_name: &str, // for bridge fallback
) -> Result<&'a ArenaValue<'a>> {
    if args.len() != 1 {
        // Multi-arg max/min isn't a pipeline top — bridge.
        let v = bridge_arith(op_name, args, context, engine)?;
        return Ok(arena.alloc(value_to_arena(&v, arena)));
    }

    let src = match resolve_iter_input(&args[0], context, engine, arena, root)? {
        ResolvedInput::Iterable(s) => s,
        ResolvedInput::Empty => return Err(Error::InvalidArguments(INVALID_ARGS.into())),
        ResolvedInput::Bridge => {
            let v = bridge_arith(op_name, args, context, engine)?;
            return Ok(arena.alloc(value_to_arena(&v, arena)));
        }
    };

    if src.is_empty() {
        return Err(Error::InvalidArguments(INVALID_ARGS.into()));
    }

    let mut best_f = init;
    let mut best_idx: Option<usize> = None;
    let len = src.len();
    for i in 0..len {
        match src.get(i) {
            Value::Number(n) => {
                if let Some(f) = n.as_f64()
                    && pick_better(f, best_f)
                {
                    best_f = f;
                    best_idx = Some(i);
                }
            }
            _ => return Err(Error::InvalidArguments(INVALID_ARGS.into())),
        }
    }

    match best_idx {
        Some(i) => {
            // Borrow the original Number to preserve integer typing — the arena
            // result is just an InputRef, no Number copy.
            Ok(arena.alloc(ArenaValue::InputRef(src.get(i))))
        }
        None => Ok(arena.alloc(ArenaValue::Null)),
    }
}

/// Arena-mode max(single_array_arg).
#[inline]
pub(crate) fn evaluate_max_arena<'a>(
    args: &[CompiledNode],
    context: &mut ContextStack,
    engine: &DataLogic,
    arena: &'a Bump,
    root: &'a Value,
) -> Result<&'a ArenaValue<'a>> {
    arena_min_max(
        args,
        context,
        engine,
        arena,
        root,
        f64::NEG_INFINITY,
        |c, b| c > b,
        "max",
    )
}

/// Arena-mode min(single_array_arg).
#[inline]
pub(crate) fn evaluate_min_arena<'a>(
    args: &[CompiledNode],
    context: &mut ContextStack,
    engine: &DataLogic,
    arena: &'a Bump,
    root: &'a Value,
) -> Result<&'a ArenaValue<'a>> {
    arena_min_max(
        args,
        context,
        engine,
        arena,
        root,
        f64::INFINITY,
        |c, b| c < b,
        "min",
    )
}

/// Arena-mode +(single_array_arg) — sum over array. Multi-arg form bridges.
#[inline]
pub(crate) fn evaluate_add_arena<'a>(
    args: &[CompiledNode],
    context: &mut ContextStack,
    engine: &DataLogic,
    arena: &'a Bump,
    root: &'a Value,
) -> Result<&'a ArenaValue<'a>> {
    if args.len() != 1 {
        let v = bridge_arith("+", args, context, engine)?;
        return Ok(arena.alloc(value_to_arena(&v, arena)));
    }
    arena_fold(args, context, engine, arena, root, "+", 0.0, |acc, x| acc + x)
}

/// Arena-mode *(single_array_arg) — product over array. Multi-arg form bridges.
#[inline]
pub(crate) fn evaluate_multiply_arena<'a>(
    args: &[CompiledNode],
    context: &mut ContextStack,
    engine: &DataLogic,
    arena: &'a Bump,
    root: &'a Value,
) -> Result<&'a ArenaValue<'a>> {
    if args.len() != 1 {
        let v = bridge_arith("*", args, context, engine)?;
        return Ok(arena.alloc(value_to_arena(&v, arena)));
    }
    arena_fold(args, context, engine, arena, root, "*", 1.0, |acc, x| acc * x)
}

/// Generic fold for sum/product over an arena-iterable input. Preserves
/// integer typing when the result fits.
#[inline]
#[allow(clippy::too_many_arguments)] // 5 contextual + op_name/init/combine
fn arena_fold<'a>(
    args: &[CompiledNode],
    context: &mut ContextStack,
    engine: &DataLogic,
    arena: &'a Bump,
    root: &'a Value,
    op_name: &str,
    init: f64,
    combine: fn(f64, f64) -> f64,
) -> Result<&'a ArenaValue<'a>> {
    let src = match resolve_iter_input(&args[0], context, engine, arena, root)? {
        ResolvedInput::Iterable(s) => s,
        ResolvedInput::Empty => {
            // Match value-mode: empty +/* returns the identity? Existing impl
            // varies — defer to value-mode for definitive semantics.
            let v = bridge_arith(op_name, args, context, engine)?;
            return Ok(arena.alloc(value_to_arena(&v, arena)));
        }
        ResolvedInput::Bridge => {
            let v = bridge_arith(op_name, args, context, engine)?;
            return Ok(arena.alloc(value_to_arena(&v, arena)));
        }
    };

    let mut acc_f = init;
    let mut all_int = true;
    let len = src.len();
    for i in 0..len {
        match src.get(i) {
            Value::Number(n) => {
                if let Some(f) = n.as_f64() {
                    acc_f = combine(acc_f, f);
                    if all_int && n.as_i64().is_none() {
                        all_int = false;
                    }
                } else {
                    return Err(Error::InvalidArguments(INVALID_ARGS.into()));
                }
            }
            _ => return Err(Error::InvalidArguments(INVALID_ARGS.into())),
        }
    }

    let n = if all_int && acc_f.fract() == 0.0 && acc_f >= i64::MIN as f64 && acc_f <= i64::MAX as f64 {
        serde_json::Number::from(acc_f as i64)
    } else {
        serde_json::Number::from_f64(acc_f).unwrap_or_else(|| serde_json::Number::from(0))
    };
    Ok(arena.alloc(ArenaValue::Number(n)))
}

/// Bridge an arithmetic op to its value-mode implementation.
#[inline]
fn bridge_arith(
    op_name: &str,
    args: &[CompiledNode],
    context: &mut ContextStack,
    engine: &DataLogic,
) -> Result<Value> {
    match op_name {
        "max" => evaluate_max(args, context, engine),
        "min" => evaluate_min(args, context, engine),
        "+" => evaluate_add(args, context, engine),
        "*" => evaluate_multiply(args, context, engine),
        _ => unreachable!("unknown arena arith bridge: {}", op_name),
    }
}
