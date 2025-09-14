use chrono::Utc;
use serde_json::{Value, json};

use crate::datetime::{
    DataDateTime, DataDuration, extract_datetime, is_datetime_object, is_duration_object,
};
use crate::{ContextStack, Error, Evaluator, Result};

/// DatetimeOperator function - creates or parses a datetime
#[inline]
pub fn evaluate_datetime(
    args: &[Value],
    context: &mut ContextStack,
    evaluator: &dyn Evaluator,
) -> Result<Value> {
    if args.is_empty() {
        return Err(Error::InvalidArguments(
            "datetime requires an argument".to_string(),
        ));
    }

    let value = evaluator.evaluate(&args[0], context)?;

    // If it's already a datetime object, return it
    if is_datetime_object(&value) {
        return Ok(value);
    }

    // Parse string as datetime and return it as-is if valid
    // This preserves the original timezone information
    if let Value::String(s) = &value
        && DataDateTime::parse(s).is_some()
    {
        // Return the original string to preserve timezone info
        return Ok(value);
    }

    Err(Error::InvalidArguments(
        "Invalid datetime format".to_string(),
    ))
}

/// TimestampOperator function - creates or parses a duration
#[inline]
pub fn evaluate_timestamp(
    args: &[Value],
    context: &mut ContextStack,
    evaluator: &dyn Evaluator,
) -> Result<Value> {
    if args.is_empty() {
        return Err(Error::InvalidArguments(
            "timestamp requires an argument".to_string(),
        ));
    }

    let value = evaluator.evaluate(&args[0], context)?;

    // If it's already a duration object, return it
    if is_duration_object(&value) {
        return Ok(value);
    }

    // Parse string as duration
    if let Value::String(s) = &value
        && let Some(duration) = DataDuration::parse(s)
    {
        return Ok(Value::String(duration.to_string()));
    }

    Err(Error::InvalidArguments(
        "Invalid duration format".to_string(),
    ))
}

/// ParseDateOperator function - parses a date string with a format
#[inline]
pub fn evaluate_parse_date(
    args: &[Value],
    context: &mut ContextStack,
    evaluator: &dyn Evaluator,
) -> Result<Value> {
    if args.len() < 2 {
        return Err(Error::InvalidArguments(
            "parse_date requires date string and format".to_string(),
        ));
    }

    let date_str = evaluator.evaluate(&args[0], context)?;
    let format_str = evaluator.evaluate(&args[1], context)?;

    if let (Value::String(date), Value::String(format)) = (date_str, format_str) {
        // Convert JSONLogic format to chrono format
        let chrono_format = format
            .replace("yyyy", "%Y")
            .replace("MM", "%m")
            .replace("dd", "%d")
            .replace("HH", "%H")
            .replace("mm", "%M")
            .replace("ss", "%S");

        if let Some(dt) = DataDateTime::parse_with_format(&date, &chrono_format) {
            return Ok(Value::String(dt.to_iso_string()));
        }
    }

    Err(Error::InvalidArguments("Failed to parse date".to_string()))
}

/// FormatDateOperator function - formats a datetime with a format string
#[inline]
pub fn evaluate_format_date(
    args: &[Value],
    context: &mut ContextStack,
    evaluator: &dyn Evaluator,
) -> Result<Value> {
    if args.len() < 2 {
        return Err(Error::InvalidArguments(
            "format_date requires datetime and format".to_string(),
        ));
    }

    let datetime_val = evaluator.evaluate(&args[0], context)?;
    let format_str = evaluator.evaluate(&args[1], context)?;

    // Extract datetime from object or string
    let dt = if is_datetime_object(&datetime_val) {
        extract_datetime(&datetime_val)
    } else if let Value::String(s) = &datetime_val {
        DataDateTime::parse(s)
    } else {
        None
    };

    if let (Some(datetime), Value::String(format)) = (dt, format_str) {
        // Convert JSONLogic format to chrono format
        let chrono_format = if format == "z" {
            // Special case for timezone offset
            format
        } else {
            format
                .replace("yyyy", "%Y")
                .replace("MM", "%m")
                .replace("dd", "%d")
                .replace("HH", "%H")
                .replace("mm", "%M")
                .replace("ss", "%S")
        };

        return Ok(Value::String(datetime.format(&chrono_format)));
    }

    Err(Error::InvalidArguments("Failed to format date".to_string()))
}

/// DateDiffOperator function - calculates difference between two dates
#[inline]
pub fn evaluate_date_diff(
    args: &[Value],
    context: &mut ContextStack,
    evaluator: &dyn Evaluator,
) -> Result<Value> {
    if args.len() < 3 {
        return Err(Error::InvalidArguments(
            "date_diff requires two dates and a unit".to_string(),
        ));
    }

    let date1_val = evaluator.evaluate(&args[0], context)?;
    let date2_val = evaluator.evaluate(&args[1], context)?;
    let unit = evaluator.evaluate(&args[2], context)?;

    // Extract datetimes
    let dt1 = if is_datetime_object(&date1_val) {
        extract_datetime(&date1_val)
    } else if let Value::String(s) = &date1_val {
        DataDateTime::parse(s)
    } else {
        None
    };

    let dt2 = if is_datetime_object(&date2_val) {
        extract_datetime(&date2_val)
    } else if let Value::String(s) = &date2_val {
        DataDateTime::parse(s)
    } else {
        None
    };

    if let (Some(datetime1), Some(datetime2), Value::String(unit_str)) = (dt1, dt2, unit) {
        let diff = datetime1.diff_in_unit(&datetime2, &unit_str);
        return Ok(json!(diff as i64));
    }

    Err(Error::InvalidArguments(
        "Failed to calculate date difference".to_string(),
    ))
}

/// NowOperator function - returns the current datetime
#[inline]
pub fn evaluate_now(
    _args: &[Value],
    _context: &mut ContextStack,
    _evaluator: &dyn Evaluator,
) -> Result<Value> {
    // Get current UTC datetime
    let now = Utc::now();

    // Create a DataDateTime with current time
    let data_dt = DataDateTime {
        dt: now,
        original_offset: Some(0), // UTC offset
    };

    // Return as ISO string
    Ok(Value::String(data_dt.to_iso_string()))
}
