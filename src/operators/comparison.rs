use serde_json::Value;

use crate::datetime::{extract_datetime, extract_duration, is_datetime_object, is_duration_object};
use crate::value_helpers::{coerce_to_number, loose_equals, strict_equals};
use crate::{ContextStack, Evaluator, Operator, Result};

/// Equals operator (== for loose, === for strict)
pub struct EqualsOperator {
    pub strict: bool,
}

impl Operator for EqualsOperator {
    fn evaluate(
        &self,
        args: &[Value],
        context: &mut ContextStack,
        evaluator: &dyn Evaluator,
    ) -> Result<Value> {
        if args.len() < 2 {
            return Ok(Value::Bool(true));
        }

        let left = evaluator.evaluate(&args[0], context)?;
        let right = evaluator.evaluate(&args[1], context)?;

        // Handle datetime comparisons - both objects and strings
        let left_dt = if is_datetime_object(&left) {
            extract_datetime(&left)
        } else if let Value::String(s) = &left {
            crate::datetime::DataDateTime::parse(s)
        } else {
            None
        };

        let right_dt = if is_datetime_object(&right) {
            extract_datetime(&right)
        } else if let Value::String(s) = &right {
            crate::datetime::DataDateTime::parse(s)
        } else {
            None
        };

        if let (Some(dt1), Some(dt2)) = (left_dt, right_dt) {
            return Ok(Value::Bool(dt1 == dt2));
        }

        // Handle duration comparisons - both objects and strings
        let left_dur = if is_duration_object(&left) {
            extract_duration(&left)
        } else if let Value::String(s) = &left {
            crate::datetime::DataDuration::parse(s)
        } else {
            None
        };

        let right_dur = if is_duration_object(&right) {
            extract_duration(&right)
        } else if let Value::String(s) = &right {
            crate::datetime::DataDuration::parse(s)
        } else {
            None
        };

        if let (Some(dur1), Some(dur2)) = (left_dur, right_dur) {
            return Ok(Value::Bool(dur1 == dur2));
        }

        let result = if self.strict {
            strict_equals(&left, &right)
        } else {
            loose_equals(&left, &right)
        };

        Ok(Value::Bool(result))
    }
}

/// Not equals operator (!= for loose, !== for strict)
pub struct NotEqualsOperator {
    pub strict: bool,
}

impl Operator for NotEqualsOperator {
    fn evaluate(
        &self,
        args: &[Value],
        context: &mut ContextStack,
        evaluator: &dyn Evaluator,
    ) -> Result<Value> {
        if args.len() < 2 {
            return Ok(Value::Bool(false));
        }

        let left = evaluator.evaluate(&args[0], context)?;
        let right = evaluator.evaluate(&args[1], context)?;

        // Handle datetime comparisons - both objects and strings
        let left_dt = if is_datetime_object(&left) {
            extract_datetime(&left)
        } else if let Value::String(s) = &left {
            crate::datetime::DataDateTime::parse(s)
        } else {
            None
        };

        let right_dt = if is_datetime_object(&right) {
            extract_datetime(&right)
        } else if let Value::String(s) = &right {
            crate::datetime::DataDateTime::parse(s)
        } else {
            None
        };

        if let (Some(dt1), Some(dt2)) = (left_dt, right_dt) {
            return Ok(Value::Bool(dt1 != dt2));
        }

        // Handle duration comparisons - both objects and strings
        let left_dur = if is_duration_object(&left) {
            extract_duration(&left)
        } else if let Value::String(s) = &left {
            crate::datetime::DataDuration::parse(s)
        } else {
            None
        };

        let right_dur = if is_duration_object(&right) {
            extract_duration(&right)
        } else if let Value::String(s) = &right {
            crate::datetime::DataDuration::parse(s)
        } else {
            None
        };

        if let (Some(dur1), Some(dur2)) = (left_dur, right_dur) {
            return Ok(Value::Bool(dur1 != dur2));
        }

        let result = if self.strict {
            !strict_equals(&left, &right)
        } else {
            !loose_equals(&left, &right)
        };

        Ok(Value::Bool(result))
    }
}

/// Greater than operator (>)
pub struct GreaterThanOperator;

impl Operator for GreaterThanOperator {
    fn evaluate(
        &self,
        args: &[Value],
        context: &mut ContextStack,
        evaluator: &dyn Evaluator,
    ) -> Result<Value> {
        if args.len() < 2 {
            return Ok(Value::Bool(false));
        }

        let left = evaluator.evaluate(&args[0], context)?;
        let right = evaluator.evaluate(&args[1], context)?;

        // Handle datetime comparisons - both objects and strings
        let left_dt = if is_datetime_object(&left) {
            extract_datetime(&left)
        } else if let Value::String(s) = &left {
            crate::datetime::DataDateTime::parse(s)
        } else {
            None
        };

        let right_dt = if is_datetime_object(&right) {
            extract_datetime(&right)
        } else if let Value::String(s) = &right {
            crate::datetime::DataDateTime::parse(s)
        } else {
            None
        };

        if let (Some(dt1), Some(dt2)) = (left_dt, right_dt) {
            return Ok(Value::Bool(dt1 > dt2));
        }

        // Handle duration comparisons - both objects and strings
        let left_dur = if is_duration_object(&left) {
            extract_duration(&left)
        } else if let Value::String(s) = &left {
            crate::datetime::DataDuration::parse(s)
        } else {
            None
        };

        let right_dur = if is_duration_object(&right) {
            extract_duration(&right)
        } else if let Value::String(s) = &right {
            crate::datetime::DataDuration::parse(s)
        } else {
            None
        };

        if let (Some(dur1), Some(dur2)) = (left_dur, right_dur) {
            return Ok(Value::Bool(dur1 > dur2));
        }

        let result = match (coerce_to_number(&left), coerce_to_number(&right)) {
            (Some(l), Some(r)) => l > r,
            _ => false,
        };

        Ok(Value::Bool(result))
    }
}

/// Greater than or equal operator (>=)
pub struct GreaterThanEqualOperator;

impl Operator for GreaterThanEqualOperator {
    fn evaluate(
        &self,
        args: &[Value],
        context: &mut ContextStack,
        evaluator: &dyn Evaluator,
    ) -> Result<Value> {
        if args.len() < 2 {
            return Ok(Value::Bool(false));
        }

        let left = evaluator.evaluate(&args[0], context)?;
        let right = evaluator.evaluate(&args[1], context)?;

        // Handle datetime comparisons - both objects and strings
        let left_dt = if is_datetime_object(&left) {
            extract_datetime(&left)
        } else if let Value::String(s) = &left {
            crate::datetime::DataDateTime::parse(s)
        } else {
            None
        };

        let right_dt = if is_datetime_object(&right) {
            extract_datetime(&right)
        } else if let Value::String(s) = &right {
            crate::datetime::DataDateTime::parse(s)
        } else {
            None
        };

        if let (Some(dt1), Some(dt2)) = (left_dt, right_dt) {
            return Ok(Value::Bool(dt1 >= dt2));
        }

        // Handle duration comparisons - both objects and strings
        let left_dur = if is_duration_object(&left) {
            extract_duration(&left)
        } else if let Value::String(s) = &left {
            crate::datetime::DataDuration::parse(s)
        } else {
            None
        };

        let right_dur = if is_duration_object(&right) {
            extract_duration(&right)
        } else if let Value::String(s) = &right {
            crate::datetime::DataDuration::parse(s)
        } else {
            None
        };

        if let (Some(dur1), Some(dur2)) = (left_dur, right_dur) {
            return Ok(Value::Bool(dur1 >= dur2));
        }

        let result = match (coerce_to_number(&left), coerce_to_number(&right)) {
            (Some(l), Some(r)) => l >= r,
            _ => false,
        };

        Ok(Value::Bool(result))
    }
}

/// Less than operator (<) - supports variadic arguments
pub struct LessThanOperator;

impl Operator for LessThanOperator {
    fn evaluate(
        &self,
        args: &[Value],
        context: &mut ContextStack,
        evaluator: &dyn Evaluator,
    ) -> Result<Value> {
        if args.len() < 2 {
            return Ok(Value::Bool(false));
        }

        let mut prev = evaluator.evaluate(&args[0], context)?;

        for item in args.iter().skip(1) {
            let current = evaluator.evaluate(item, context)?;

            // Handle datetime comparisons - both objects and strings
            let prev_dt = if is_datetime_object(&prev) {
                extract_datetime(&prev)
            } else if let Value::String(s) = &prev {
                crate::datetime::DataDateTime::parse(s)
            } else {
                None
            };

            let curr_dt = if is_datetime_object(&current) {
                extract_datetime(&current)
            } else if let Value::String(s) = &current {
                crate::datetime::DataDateTime::parse(s)
            } else {
                None
            };

            if let (Some(dt1), Some(dt2)) = (prev_dt, curr_dt) {
                if dt1 >= dt2 {
                    return Ok(Value::Bool(false));
                }
                prev = current;
                continue;
            }

            // Handle duration comparisons - both objects and strings
            let prev_dur = if is_duration_object(&prev) {
                extract_duration(&prev)
            } else if let Value::String(s) = &prev {
                crate::datetime::DataDuration::parse(s)
            } else {
                None
            };

            let curr_dur = if is_duration_object(&current) {
                extract_duration(&current)
            } else if let Value::String(s) = &current {
                crate::datetime::DataDuration::parse(s)
            } else {
                None
            };

            if let (Some(dur1), Some(dur2)) = (prev_dur, curr_dur) {
                if dur1 >= dur2 {
                    return Ok(Value::Bool(false));
                }
                prev = current;
                continue;
            }

            let result = match (coerce_to_number(&prev), coerce_to_number(&current)) {
                (Some(l), Some(r)) => l < r,
                _ => return Ok(Value::Bool(false)),
            };

            if !result {
                return Ok(Value::Bool(false));
            }

            prev = current;
        }

        Ok(Value::Bool(true))
    }
}

/// Less than or equal operator (<=) - supports variadic arguments
pub struct LessThanEqualOperator;

impl Operator for LessThanEqualOperator {
    fn evaluate(
        &self,
        args: &[Value],
        context: &mut ContextStack,
        evaluator: &dyn Evaluator,
    ) -> Result<Value> {
        if args.len() < 2 {
            return Ok(Value::Bool(false));
        }

        let mut prev = evaluator.evaluate(&args[0], context)?;

        for item in args.iter().skip(1) {
            let current = evaluator.evaluate(item, context)?;

            // Handle datetime comparisons - both objects and strings
            let prev_dt = if is_datetime_object(&prev) {
                extract_datetime(&prev)
            } else if let Value::String(s) = &prev {
                crate::datetime::DataDateTime::parse(s)
            } else {
                None
            };

            let curr_dt = if is_datetime_object(&current) {
                extract_datetime(&current)
            } else if let Value::String(s) = &current {
                crate::datetime::DataDateTime::parse(s)
            } else {
                None
            };

            if let (Some(dt1), Some(dt2)) = (prev_dt, curr_dt) {
                if dt1 > dt2 {
                    return Ok(Value::Bool(false));
                }
                prev = current;
                continue;
            }

            // Handle duration comparisons - both objects and strings
            let prev_dur = if is_duration_object(&prev) {
                extract_duration(&prev)
            } else if let Value::String(s) = &prev {
                crate::datetime::DataDuration::parse(s)
            } else {
                None
            };

            let curr_dur = if is_duration_object(&current) {
                extract_duration(&current)
            } else if let Value::String(s) = &current {
                crate::datetime::DataDuration::parse(s)
            } else {
                None
            };

            if let (Some(dur1), Some(dur2)) = (prev_dur, curr_dur) {
                if dur1 > dur2 {
                    return Ok(Value::Bool(false));
                }
                prev = current;
                continue;
            }

            let result = match (coerce_to_number(&prev), coerce_to_number(&current)) {
                (Some(l), Some(r)) => l <= r,
                _ => return Ok(Value::Bool(false)),
            };

            if !result {
                return Ok(Value::Bool(false));
            }

            prev = current;
        }

        Ok(Value::Bool(true))
    }
}
