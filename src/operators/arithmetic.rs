use serde_json::Value;

use crate::datetime::{extract_datetime, extract_duration, is_datetime_object, is_duration_object};
use crate::value_helpers::{coerce_to_number, try_coerce_to_integer};
use crate::{ContextStack, Error, Evaluator, Operator, Result};

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
                    int_sum = int_sum.saturating_add(i);
                }
                float_sum += i as f64;
            } else if let Some(f) = coerce_to_number(&value) {
                all_integers = false;
                float_sum += f;
            } else {
                return Ok(Value::Null);
            }
        }

        // Return integer if all inputs were integers, otherwise float
        if all_integers {
            Ok(Value::Number(int_sum.into()))
        } else {
            Ok(serde_json::Number::from_f64(float_sum)
                .map(Value::Number)
                .unwrap_or(Value::Null))
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
            return Ok(Value::Number(0.into()));
        }

        let first = evaluator.evaluate(&args[0], context)?;

        if args.len() == 1 {
            // Negation
            if let Value::Number(n) = &first {
                if let Some(i) = n.as_i64() {
                    return Ok(Value::Number((-i).into()));
                } else if let Some(f) = n.as_f64() {
                    return Ok(serde_json::Number::from_f64(-f)
                        .map(Value::Number)
                        .unwrap_or(Value::Null));
                }
            }
            let first_num = coerce_to_number(&first)
                .ok_or_else(|| Error::TypeError("Cannot convert to number".to_string()))?;
            Ok(serde_json::Number::from_f64(-first_num)
                .map(Value::Number)
                .unwrap_or(Value::Null))
        } else {
            // Subtraction
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
                return Ok(Value::Number((i1 - i2).into()));
            }

            let first_num = coerce_to_number(&first)
                .ok_or_else(|| Error::TypeError("Cannot convert to number".to_string()))?;
            let second_num = coerce_to_number(&second)
                .ok_or_else(|| Error::TypeError("Cannot convert to number".to_string()))?;

            Ok(serde_json::Number::from_f64(first_num - second_num)
                .map(Value::Number)
                .unwrap_or(Value::Null))
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
                return Ok(Value::Null);
            }
        }

        if all_integers {
            Ok(Value::Number(int_product.into()))
        } else {
            Ok(serde_json::Number::from_f64(float_product)
                .map(Value::Number)
                .unwrap_or(Value::Null))
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
        if args.len() < 2 {
            return Ok(Value::Null);
        }

        let first = evaluator.evaluate(&args[0], context)?;
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
            // Check if division is exact (no remainder)
            if i1 % i2 == 0 {
                return Ok(Value::Number((i1 / i2).into()));
            }
        }

        let first_num = coerce_to_number(&first)
            .ok_or_else(|| Error::TypeError("Cannot convert to number".to_string()))?;
        let second_num = coerce_to_number(&second)
            .ok_or_else(|| Error::TypeError("Cannot convert to number".to_string()))?;

        if second_num == 0.0 {
            return Err(Error::Thrown(serde_json::json!({"type": "NaN"})));
        }

        Ok(serde_json::Number::from_f64(first_num / second_num)
            .map(Value::Number)
            .unwrap_or(Value::Null))
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
        if args.len() < 2 {
            return Ok(Value::Null);
        }

        let first = evaluator.evaluate(&args[0], context)?;
        let second = evaluator.evaluate(&args[1], context)?;

        // Check if both are integers
        if let (Value::Number(n1), Value::Number(n2)) = (&first, &second)
            && let (Some(i1), Some(i2)) = (n1.as_i64(), n2.as_i64())
        {
            if i2 == 0 {
                return Err(Error::Thrown(serde_json::json!({"type": "NaN"})));
            }
            return Ok(Value::Number((i1 % i2).into()));
        }

        let first_num = coerce_to_number(&first)
            .ok_or_else(|| Error::TypeError("Cannot convert to number".to_string()))?;
        let second_num = coerce_to_number(&second)
            .ok_or_else(|| Error::TypeError("Cannot convert to number".to_string()))?;

        if second_num == 0.0 {
            return Err(Error::Thrown(serde_json::json!({"type": "NaN"})));
        }

        Ok(serde_json::Number::from_f64(first_num % second_num)
            .map(Value::Number)
            .unwrap_or(Value::Null))
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
            return Ok(Value::Null);
        }

        let mut max_value: Option<Value> = None;
        let mut max_num = f64::NEG_INFINITY;

        for arg in args {
            let value = evaluator.evaluate(arg, context)?;
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
            return Ok(Value::Null);
        }

        let mut min_value: Option<Value> = None;
        let mut min_num = f64::INFINITY;

        for arg in args {
            let value = evaluator.evaluate(arg, context)?;
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
