use serde_json::Value;

use crate::datetime::{extract_datetime, extract_duration, is_datetime_object, is_duration_object};
use crate::value_helpers::{coerce_to_number, try_coerce_to_integer};
use crate::{ContextStack, Error, Evaluator, Operator, Result};

/// Helper to convert float to integer if it's a whole number
fn number_value(f: f64) -> Value {
    if f.is_finite() && f.floor() == f && f >= i64::MIN as f64 && f <= i64::MAX as f64 {
        Value::Number((f as i64).into())
    } else {
        serde_json::Number::from_f64(f)
            .map(Value::Number)
            .unwrap_or(Value::Null)
    }
}

/// Addition operator (+) - variadic
pub struct AddOperator;

impl Operator for AddOperator {
    fn evaluate(
        &self,
        args: &[Value],
        context: &mut ContextStack,
        evaluator: &dyn Evaluator,
    ) -> Result<Value> {
        if args.is_empty() {
            return Ok(Value::Number(0.into()));
        }

        // Special case: single array argument - sum all elements
        if args.len() == 1 {
            // Check if the argument is a literal array (not from an operator)
            if let Value::Array(_) = &args[0] {
                // Literal array as argument - this is invalid for addition
                return Err(Error::Thrown(serde_json::json!({"type": "NaN"})));
            }

            let value = evaluator.evaluate(&args[0], context)?;
            if let Value::Array(arr) = value {
                // Array from operator evaluation - sum the elements
                if arr.is_empty() {
                    return Ok(Value::Number(0.into())); // Identity element for addition
                }
                // Don't recursively call evaluate - that would treat the array as literal
                // Instead, evaluate each element and sum them
                let mut all_integers = true;
                let mut int_sum: i64 = 0;
                let mut float_sum = 0.0;

                for elem in &arr {
                    // Array elements are already evaluated values
                    if let Some(i) = try_coerce_to_integer(elem) {
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
                            float_sum += i as f64;
                        }
                    } else if let Some(f) = coerce_to_number(elem) {
                        all_integers = false;
                        float_sum += f;
                    } else {
                        return Err(Error::Thrown(serde_json::json!({"type": "NaN"})));
                    }
                }

                return if all_integers {
                    Ok(Value::Number(int_sum.into()))
                } else {
                    Ok(number_value(float_sum))
                };
            }
        }

        // Special case for datetime/duration arithmetic
        if args.len() == 2 {
            let first = evaluator.evaluate(&args[0], context)?;
            let second = evaluator.evaluate(&args[1], context)?;

            // DateTime + Duration
            let first_dt = if is_datetime_object(&first) {
                extract_datetime(&first)
            } else if let Value::String(s) = &first {
                crate::datetime::DataDateTime::parse(s)
            } else {
                None
            };

            let second_dur = if is_duration_object(&second) {
                extract_duration(&second)
            } else if let Value::String(s) = &second {
                crate::datetime::DataDuration::parse(s)
            } else {
                None
            };

            if let (Some(dt), Some(dur)) = (first_dt, second_dur) {
                let result = dt.add_duration(&dur);
                return Ok(Value::String(result.to_iso_string()));
            }

            // Duration + Duration
            let first_dur = if is_duration_object(&first) {
                extract_duration(&first)
            } else if let Value::String(s) = &first {
                crate::datetime::DataDuration::parse(s)
            } else {
                None
            };

            let second_dur2 = if is_duration_object(&second) {
                extract_duration(&second)
            } else if let Value::String(s) = &second {
                crate::datetime::DataDuration::parse(s)
            } else {
                None
            };

            if let (Some(dur1), Some(dur2)) = (first_dur, second_dur2) {
                let result = dur1.add(&dur2);
                return Ok(Value::String(result.to_string()));
            }
        }

        // Regular numeric addition
        // Check if all values are integers
        let mut all_integers = true;
        let mut int_sum: i64 = 0;
        let mut float_sum = 0.0;

        for arg in args {
            let value = evaluator.evaluate(arg, context)?;

            // Try integer coercion first
            if let Some(i) = try_coerce_to_integer(&value) {
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
                    float_sum += i as f64;
                }
            } else if let Some(f) = coerce_to_number(&value) {
                all_integers = false;
                float_sum += f;
            } else {
                return Err(Error::Thrown(serde_json::json!({"type": "NaN"})));
            }
        }

        // Return integer if all inputs were integers, otherwise float
        if all_integers {
            Ok(Value::Number(int_sum.into()))
        } else {
            Ok(number_value(float_sum))
        }
    }
}

/// Subtraction operator (-) - also handles negation
pub struct SubtractOperator;

impl Operator for SubtractOperator {
    fn evaluate(
        &self,
        args: &[Value],
        context: &mut ContextStack,
        evaluator: &dyn Evaluator,
    ) -> Result<Value> {
        if args.is_empty() {
            return Err(Error::InvalidArguments("Invalid Arguments".to_string()));
        }

        let first = evaluator.evaluate(&args[0], context)?;

        if args.len() == 1 {
            // Check if it's an array - subtract all elements
            if let Value::Array(arr) = first {
                if arr.is_empty() {
                    return Err(Error::InvalidArguments("Invalid Arguments".to_string()));
                }
                // Recursively call ourselves with the array elements
                return self.evaluate(&arr, context, evaluator);
            }

            // Negation
            if let Value::Number(n) = &first {
                if let Some(i) = n.as_i64() {
                    return Ok(Value::Number((-i).into()));
                } else if let Some(f) = n.as_f64() {
                    return Ok(number_value(-f));
                }
            }
            let first_num = coerce_to_number(&first)
                .ok_or_else(|| Error::Thrown(serde_json::json!({"type": "NaN"})))?;
            Ok(number_value(-first_num))
        } else if args.len() == 2 {
            // Special case for datetime/duration arithmetic
            let second = evaluator.evaluate(&args[1], context)?;

            // Try to parse as datetime/duration
            let first_dt = if is_datetime_object(&first) {
                extract_datetime(&first)
            } else if let Value::String(s) = &first {
                crate::datetime::DataDateTime::parse(s)
            } else {
                None
            };

            let second_dt = if is_datetime_object(&second) {
                extract_datetime(&second)
            } else if let Value::String(s) = &second {
                crate::datetime::DataDateTime::parse(s)
            } else {
                None
            };

            let first_dur = if is_duration_object(&first) {
                extract_duration(&first)
            } else if let Value::String(s) = &first {
                crate::datetime::DataDuration::parse(s)
            } else {
                None
            };

            let second_dur = if is_duration_object(&second) {
                extract_duration(&second)
            } else if let Value::String(s) = &second {
                crate::datetime::DataDuration::parse(s)
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

            // Try integer coercion first for both operands
            if let (Some(i1), Some(i2)) = (
                try_coerce_to_integer(&first),
                try_coerce_to_integer(&second),
            ) {
                // Check for overflow in subtraction
                match i1.checked_sub(i2) {
                    Some(result) => return Ok(Value::Number(result.into())),
                    None => {
                        // Overflow, fall through to float calculation
                    }
                }
            }

            let first_num = coerce_to_number(&first)
                .ok_or_else(|| Error::Thrown(serde_json::json!({"type": "NaN"})))?;
            let second_num = coerce_to_number(&second)
                .ok_or_else(|| Error::Thrown(serde_json::json!({"type": "NaN"})))?;

            Ok(number_value(first_num - second_num))
        } else {
            // Variadic subtraction (3+ arguments)
            // Check if all values are integers
            let mut all_integers = true;
            let mut int_result = if let Some(i) = try_coerce_to_integer(&first) {
                i
            } else {
                all_integers = false;
                0
            };
            let mut float_result = if let Some(f) = coerce_to_number(&first) {
                f
            } else {
                return Ok(Value::Null);
            };

            // Subtract remaining arguments
            for item in args.iter().skip(1) {
                let value = evaluator.evaluate(item, context)?;

                if all_integers {
                    if let Some(i) = try_coerce_to_integer(&value) {
                        // Check for overflow in subtraction
                        match int_result.checked_sub(i) {
                            Some(result) => int_result = result,
                            None => {
                                // Overflow detected, switch to float
                                all_integers = false;
                                float_result = int_result as f64 - i as f64;
                            }
                        }
                    } else if let Some(f) = coerce_to_number(&value) {
                        all_integers = false;
                        float_result = int_result as f64 - f;
                    } else {
                        return Ok(Value::Null);
                    }
                } else if let Some(f) = coerce_to_number(&value) {
                    float_result -= f;
                } else {
                    return Ok(Value::Null);
                }
            }

            if all_integers {
                Ok(Value::Number(int_result.into()))
            } else {
                Ok(number_value(float_result))
            }
        }
    }
}

/// Multiplication operator (*) - variadic
pub struct MultiplyOperator;

impl Operator for MultiplyOperator {
    fn evaluate(
        &self,
        args: &[Value],
        context: &mut ContextStack,
        evaluator: &dyn Evaluator,
    ) -> Result<Value> {
        if args.is_empty() {
            return Ok(Value::Number(1.into()));
        }

        // Special case: single array argument - multiply all elements
        if args.len() == 1 {
            // Check if the argument is a literal array (not from an operator)
            if let Value::Array(_) = &args[0] {
                // Literal array as argument - this is invalid for multiplication
                return Err(Error::Thrown(serde_json::json!({"type": "NaN"})));
            }

            let value = evaluator.evaluate(&args[0], context)?;
            if let Value::Array(arr) = value {
                // Array from operator evaluation - multiply the elements
                if arr.is_empty() {
                    return Ok(Value::Number(1.into())); // Identity element for multiplication
                }
                // Don't recursively call evaluate - that would treat the array as literal
                // Instead, evaluate each element and multiply them
                let mut all_integers = true;
                let mut int_product: i64 = 1;
                let mut float_product = 1.0;

                for elem in &arr {
                    // Array elements are already evaluated values
                    if let Some(i) = try_coerce_to_integer(elem) {
                        if all_integers {
                            match int_product.checked_mul(i) {
                                Some(p) => int_product = p,
                                None => {
                                    all_integers = false;
                                    float_product = int_product as f64 * i as f64;
                                }
                            }
                        } else {
                            float_product *= i as f64;
                        }
                    } else if let Some(f) = coerce_to_number(elem) {
                        if all_integers {
                            float_product = int_product as f64 * f;
                        } else {
                            float_product *= f;
                        }
                        all_integers = false;
                    } else {
                        return Err(Error::Thrown(serde_json::json!({"type": "NaN"})));
                    }
                }

                return if all_integers {
                    Ok(Value::Number(int_product.into()))
                } else {
                    Ok(number_value(float_product))
                };
            }
        }

        // Special case for duration * number
        if args.len() == 2 {
            let first = evaluator.evaluate(&args[0], context)?;
            let second = evaluator.evaluate(&args[1], context)?;

            // Duration * Number
            let first_dur = if is_duration_object(&first) {
                extract_duration(&first)
            } else if let Value::String(s) = &first {
                crate::datetime::DataDuration::parse(s)
            } else {
                None
            };

            if let Some(dur) = first_dur
                && let Some(factor) = coerce_to_number(&second)
            {
                let result = dur.multiply(factor);
                return Ok(Value::String(result.to_string()));
            }

            // Number * Duration
            let second_dur = if is_duration_object(&second) {
                extract_duration(&second)
            } else if let Value::String(s) = &second {
                crate::datetime::DataDuration::parse(s)
            } else {
                None
            };

            if let Some(dur) = second_dur
                && let Some(factor) = coerce_to_number(&first)
            {
                let result = dur.multiply(factor);
                return Ok(Value::String(result.to_string()));
            }
        }

        // Regular numeric multiplication
        // Check if all values are integers
        let mut all_integers = true;
        let mut int_product: i64 = 1;
        let mut float_product = 1.0;

        for arg in args {
            let value = evaluator.evaluate(arg, context)?;

            // Try integer coercion first
            if let Some(i) = try_coerce_to_integer(&value) {
                if all_integers {
                    match int_product.checked_mul(i) {
                        Some(p) => int_product = p,
                        None => {
                            all_integers = false;
                            float_product = int_product as f64 * i as f64;
                        }
                    }
                } else {
                    float_product *= i as f64;
                }
            } else if let Some(f) = coerce_to_number(&value) {
                if all_integers {
                    float_product = int_product as f64 * f;
                } else {
                    float_product *= f;
                }
                all_integers = false;
            } else {
                return Err(Error::Thrown(serde_json::json!({"type": "NaN"})));
            }
        }

        if all_integers {
            Ok(Value::Number(int_product.into()))
        } else {
            Ok(number_value(float_product))
        }
    }
}

/// Division operator (/)
pub struct DivideOperator;

impl Operator for DivideOperator {
    fn evaluate(
        &self,
        args: &[Value],
        context: &mut ContextStack,
        evaluator: &dyn Evaluator,
    ) -> Result<Value> {
        if args.is_empty() {
            return Err(Error::InvalidArguments("Invalid Arguments".to_string()));
        }

        // Special case: single argument
        if args.len() == 1 {
            eprintln!("DEBUG max: Single argument case");
            let value = evaluator.evaluate(&args[0], context)?;
            eprintln!("DEBUG max: Evaluated to: {:?}", value);

            // If it's an array, divide all elements sequentially
            if let Value::Array(arr) = value {
                if arr.is_empty() {
                    return Err(Error::InvalidArguments("Invalid Arguments".to_string()));
                }
                // Recursively call ourselves with the array elements
                return self.evaluate(&arr, context, evaluator);
            }

            // Single non-array argument: 1 / value
            let num = coerce_to_number(&value)
                .ok_or_else(|| Error::Thrown(serde_json::json!({"type": "NaN"})))?;

            if num == 0.0 {
                return Err(Error::Thrown(serde_json::json!({"type": "NaN"})));
            }

            // Try to preserve integer type with overflow check
            if let Some(i) = try_coerce_to_integer(&value)
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

        let first = evaluator.evaluate(&args[0], context)?;

        if args.len() == 2 {
            let second = evaluator.evaluate(&args[1], context)?;

            // Duration / Number
            let first_dur = if is_duration_object(&first) {
                extract_duration(&first)
            } else if let Value::String(s) = &first {
                crate::datetime::DataDuration::parse(s)
            } else {
                None
            };

            if let Some(dur) = first_dur
                && let Some(divisor) = coerce_to_number(&second)
            {
                if divisor == 0.0 {
                    return Err(Error::Thrown(serde_json::json!({"type": "NaN"})));
                }
                let result = dur.divide(divisor);
                return Ok(Value::String(result.to_string()));
            }

            // Try integer division first if both can be coerced to integers
            if let (Some(i1), Some(i2)) = (
                try_coerce_to_integer(&first),
                try_coerce_to_integer(&second),
            ) {
                if i2 == 0 {
                    return Err(Error::Thrown(serde_json::json!({"type": "NaN"})));
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

            let first_num = coerce_to_number(&first)
                .ok_or_else(|| Error::Thrown(serde_json::json!({"type": "NaN"})))?;
            let second_num = coerce_to_number(&second)
                .ok_or_else(|| Error::Thrown(serde_json::json!({"type": "NaN"})))?;

            if second_num == 0.0 {
                return Err(Error::Thrown(serde_json::json!({"type": "NaN"})));
            }

            Ok(number_value(first_num / second_num))
        } else {
            // Variadic division (3+ arguments)
            // Try to maintain integer type if possible
            let mut all_integers = true;
            let mut int_result = if let Some(i) = try_coerce_to_integer(&first) {
                i
            } else {
                all_integers = false;
                0
            };
            let mut float_result = coerce_to_number(&first)
                .ok_or_else(|| Error::Thrown(serde_json::json!({"type": "NaN"})))?;

            for item in args.iter().skip(1) {
                let value = evaluator.evaluate(item, context)?;

                if all_integers {
                    if let Some(divisor) = try_coerce_to_integer(&value) {
                        if divisor == 0 {
                            return Err(Error::Thrown(serde_json::json!({"type": "NaN"})));
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
                    } else if let Some(divisor) = coerce_to_number(&value) {
                        if divisor == 0.0 {
                            return Err(Error::Thrown(serde_json::json!({"type": "NaN"})));
                        }
                        all_integers = false;
                        float_result = int_result as f64 / divisor;
                    } else {
                        return Ok(Value::Null);
                    }
                } else {
                    let divisor = coerce_to_number(&value)
                        .ok_or_else(|| Error::Thrown(serde_json::json!({"type": "NaN"})))?;
                    if divisor == 0.0 {
                        return Err(Error::Thrown(serde_json::json!({"type": "NaN"})));
                    }
                    float_result /= divisor;
                }
            }

            if all_integers {
                Ok(Value::Number(int_result.into()))
            } else {
                Ok(number_value(float_result))
            }
        }
    }
}

/// Modulo operator (%)
pub struct ModuloOperator;

impl Operator for ModuloOperator {
    fn evaluate(
        &self,
        args: &[Value],
        context: &mut ContextStack,
        evaluator: &dyn Evaluator,
    ) -> Result<Value> {
        if args.is_empty() {
            return Err(Error::InvalidArguments("Invalid Arguments".to_string()));
        }

        // Special case: single array argument - modulo all elements sequentially
        if args.len() == 1 {
            let value = evaluator.evaluate(&args[0], context)?;
            if let Value::Array(arr) = value {
                if arr.is_empty() || arr.len() < 2 {
                    return Err(Error::InvalidArguments("Invalid Arguments".to_string()));
                }
                // Recursively call ourselves with the array elements
                return self.evaluate(&arr, context, evaluator);
            }
            return Err(Error::InvalidArguments("Invalid Arguments".to_string()));
        }

        if args.len() < 2 {
            return Err(Error::InvalidArguments("Invalid Arguments".to_string()));
        }

        let first = evaluator.evaluate(&args[0], context)?;

        if args.len() == 2 {
            let second = evaluator.evaluate(&args[1], context)?;

            // Check if both are integers
            if let (Value::Number(n1), Value::Number(n2)) = (&first, &second)
                && let (Some(i1), Some(i2)) = (n1.as_i64(), n2.as_i64())
            {
                if i2 == 0 {
                    return Err(Error::Thrown(serde_json::json!({"type": "NaN"})));
                }
                // Special case: i64::MIN % -1 would overflow in some contexts
                if i1 == i64::MIN && i2 == -1 {
                    return Ok(Value::Number(0.into()));
                }
                return Ok(Value::Number((i1 % i2).into()));
            }

            let first_num = coerce_to_number(&first)
                .ok_or_else(|| Error::Thrown(serde_json::json!({"type": "NaN"})))?;
            let second_num = coerce_to_number(&second)
                .ok_or_else(|| Error::Thrown(serde_json::json!({"type": "NaN"})))?;

            if second_num == 0.0 {
                return Err(Error::Thrown(serde_json::json!({"type": "NaN"})));
            }

            Ok(number_value(first_num % second_num))
        } else {
            // Variadic modulo (3+ arguments)
            let mut result = coerce_to_number(&first)
                .ok_or_else(|| Error::Thrown(serde_json::json!({"type": "NaN"})))?;

            for item in args.iter().skip(1) {
                let value = evaluator.evaluate(item, context)?;
                let num = coerce_to_number(&value)
                    .ok_or_else(|| Error::Thrown(serde_json::json!({"type": "NaN"})))?;

                if num == 0.0 {
                    return Err(Error::Thrown(serde_json::json!({"type": "NaN"})));
                }

                result %= num;
            }

            Ok(number_value(result))
        }
    }
}

/// Max operator - variadic
pub struct MaxOperator;

impl Operator for MaxOperator {
    fn evaluate(
        &self,
        args: &[Value],
        context: &mut ContextStack,
        evaluator: &dyn Evaluator,
    ) -> Result<Value> {
        if args.is_empty() {
            return Err(Error::InvalidArguments("Invalid Arguments".to_string()));
        }

        // Special case: single argument
        if args.len() == 1 {
            let value = evaluator.evaluate(&args[0], context)?;

            // If evaluation produced an array, find max of its elements
            // But if the original arg was a literal array, that's an error
            if let Value::Array(arr) = value {
                // Check if original argument was a literal array
                if matches!(&args[0], Value::Array(_)) {
                    return Err(Error::InvalidArguments("Invalid Arguments".to_string()));
                }

                if arr.is_empty() {
                    return Err(Error::InvalidArguments("Invalid Arguments".to_string()));
                }

                // Process array elements directly instead of recursing
                let mut max_value: Option<Value> = None;
                let mut max_num = f64::NEG_INFINITY;

                for elem in arr {
                    // Array elements are already evaluated, just check they're numeric
                    if !matches!(elem, Value::Number(_)) {
                        return Err(Error::InvalidArguments("Invalid Arguments".to_string()));
                    }

                    if let Some(n) = coerce_to_number(&elem)
                        && n > max_num
                    {
                        max_num = n;
                        max_value = Some(elem);
                    }
                }

                return Ok(max_value.unwrap_or(Value::Null));
            }

            // Single non-array argument - check if it's numeric
            if !matches!(value, Value::Number(_)) {
                return Err(Error::InvalidArguments("Invalid Arguments".to_string()));
            }
            return Ok(value);
        }

        let mut max_value: Option<Value> = None;
        let mut max_num = f64::NEG_INFINITY;

        for arg in args {
            let value = evaluator.evaluate(arg, context)?;

            // Only accept numeric values
            if !matches!(value, Value::Number(_)) {
                return Err(Error::InvalidArguments("Invalid Arguments".to_string()));
            }

            if let Some(n) = coerce_to_number(&value)
                && n > max_num
            {
                max_num = n;
                max_value = Some(value);
            }
        }

        // Return the actual value that was max (preserving integer type)
        Ok(max_value.unwrap_or(Value::Null))
    }
}

/// Min operator - variadic
pub struct MinOperator;

impl Operator for MinOperator {
    fn evaluate(
        &self,
        args: &[Value],
        context: &mut ContextStack,
        evaluator: &dyn Evaluator,
    ) -> Result<Value> {
        if args.is_empty() {
            return Err(Error::InvalidArguments("Invalid Arguments".to_string()));
        }

        // Special case: single argument
        if args.len() == 1 {
            let value = evaluator.evaluate(&args[0], context)?;

            // If evaluation produced an array, find min of its elements
            // But if the original arg was a literal array, that's an error
            if let Value::Array(arr) = value {
                // Check if original argument was a literal array
                if matches!(&args[0], Value::Array(_)) {
                    return Err(Error::InvalidArguments("Invalid Arguments".to_string()));
                }

                if arr.is_empty() {
                    return Err(Error::InvalidArguments("Invalid Arguments".to_string()));
                }

                // Process array elements directly instead of recursing
                let mut min_value: Option<Value> = None;
                let mut min_num = f64::INFINITY;

                for elem in arr {
                    // Array elements are already evaluated, just check they're numeric
                    if !matches!(elem, Value::Number(_)) {
                        return Err(Error::InvalidArguments("Invalid Arguments".to_string()));
                    }

                    if let Some(n) = coerce_to_number(&elem)
                        && n < min_num
                    {
                        min_num = n;
                        min_value = Some(elem);
                    }
                }

                return Ok(min_value.unwrap_or(Value::Null));
            }

            // Single non-array argument - check if it's numeric
            if !matches!(value, Value::Number(_)) {
                return Err(Error::InvalidArguments("Invalid Arguments".to_string()));
            }
            return Ok(value);
        }

        let mut min_value: Option<Value> = None;
        let mut min_num = f64::INFINITY;

        for arg in args {
            let value = evaluator.evaluate(arg, context)?;

            // Only accept numeric values
            if !matches!(value, Value::Number(_)) {
                return Err(Error::InvalidArguments("Invalid Arguments".to_string()));
            }

            if let Some(n) = coerce_to_number(&value)
                && n < min_num
            {
                min_num = n;
                min_value = Some(value);
            }
        }

        // Return the actual value that was min (preserving integer type)
        Ok(min_value.unwrap_or(Value::Null))
    }
}
