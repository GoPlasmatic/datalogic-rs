//! Tests for custom operators with preserve_structure mode

use chrono::{DateTime, Timelike};
use datalogic_rs::{ContextStack, DataLogic, Error, Evaluator, Operator, Result};
use serde_json::{json, Value};

/// Custom operator that checks if a datetime is during nighttime hours
struct IsNightOperator;

impl Operator for IsNightOperator {
    fn evaluate(
        &self,
        args: &[Value],
        context: &mut ContextStack,
        evaluator: &dyn Evaluator,
    ) -> Result<Value> {
        if args.len() != 1 {
            return Err(Error::InvalidArguments(
                "Expected exactly one argument".to_string(),
            ));
        }

        let evaluated_arg = evaluator.evaluate(&args[0], context)?;
        let datetime = parse_datetime(&evaluated_arg).ok_or_else(|| {
            Error::InvalidArguments("Invalid datetime argument".to_string())
        })?;

        let hour = datetime.hour();
        let is_night = hour >= 19 || hour < 7;
        Ok(json!(is_night))
    }
}

fn parse_datetime(value: &Value) -> Option<DateTime<chrono::Utc>> {
    if let Value::Object(map) = value {
        if let Some(Value::String(datetime_str)) = map.get("datetime") {
            if let Ok(dt) = DateTime::parse_from_rfc3339(datetime_str) {
                return Some(dt.with_timezone(&chrono::Utc));
            }
        }
    }
    if let Value::String(datetime_str) = value {
        if let Ok(dt) = DateTime::parse_from_rfc3339(datetime_str) {
            return Some(dt.with_timezone(&chrono::Utc));
        }
    }
    None
}

#[test]
fn test_is_night_nighttime() {
    let mut engine = DataLogic::new();
    engine.add_operator("is_night".to_string(), Box::new(IsNightOperator));

    // 8 PM should be nighttime
    let logic = json!({"is_night": {"datetime": "2022-07-06T20:00:00Z"}});
    let compiled = engine.compile(&logic).unwrap();
    let result = engine.evaluate_owned(&compiled, json!({})).unwrap();
    assert_eq!(result, json!(true));

    // 11:59 PM should be nighttime
    let logic = json!({"is_night": {"datetime": "2022-07-06T23:59:59Z"}});
    let compiled = engine.compile(&logic).unwrap();
    let result = engine.evaluate_owned(&compiled, json!({})).unwrap();
    assert_eq!(result, json!(true));

    // 3 AM should be nighttime
    let logic = json!({"is_night": {"datetime": "2022-07-06T03:00:00Z"}});
    let compiled = engine.compile(&logic).unwrap();
    let result = engine.evaluate_owned(&compiled, json!({})).unwrap();
    assert_eq!(result, json!(true));
}

#[test]
fn test_is_night_daytime() {
    let mut engine = DataLogic::new();
    engine.add_operator("is_night".to_string(), Box::new(IsNightOperator));

    // 8 AM should not be nighttime
    let logic = json!({"is_night": {"datetime": "2022-07-06T08:00:00Z"}});
    let compiled = engine.compile(&logic).unwrap();
    let result = engine.evaluate_owned(&compiled, json!({})).unwrap();
    assert_eq!(result, json!(false));

    // Noon should not be nighttime
    let logic = json!({"is_night": {"datetime": "2022-07-06T12:00:00Z"}});
    let compiled = engine.compile(&logic).unwrap();
    let result = engine.evaluate_owned(&compiled, json!({})).unwrap();
    assert_eq!(result, json!(false));

    // 3 PM should not be nighttime
    let logic = json!({"is_night": {"datetime": "2022-07-06T15:00:00Z"}});
    let compiled = engine.compile(&logic).unwrap();
    let result = engine.evaluate_owned(&compiled, json!({})).unwrap();
    assert_eq!(result, json!(false));
}

#[test]
fn test_is_night_boundaries() {
    let mut engine = DataLogic::new();
    engine.add_operator("is_night".to_string(), Box::new(IsNightOperator));

    // 7 PM exactly should be nighttime
    let logic = json!({"is_night": {"datetime": "2022-07-06T19:00:00Z"}});
    let compiled = engine.compile(&logic).unwrap();
    let result = engine.evaluate_owned(&compiled, json!({})).unwrap();
    assert_eq!(result, json!(true));

    // 7 AM exactly should not be nighttime
    let logic = json!({"is_night": {"datetime": "2022-07-06T07:00:00Z"}});
    let compiled = engine.compile(&logic).unwrap();
    let result = engine.evaluate_owned(&compiled, json!({})).unwrap();
    assert_eq!(result, json!(false));

    // 6:59 AM should be nighttime
    let logic = json!({"is_night": {"datetime": "2022-07-06T06:59:59Z"}});
    let compiled = engine.compile(&logic).unwrap();
    let result = engine.evaluate_owned(&compiled, json!({})).unwrap();
    assert_eq!(result, json!(true));
}

#[test]
fn test_is_night_with_string() {
    let mut engine = DataLogic::new();
    engine.add_operator("is_night".to_string(), Box::new(IsNightOperator));

    // String datetime - 9 PM should be nighttime
    let logic = json!({"is_night": "2022-07-06T21:00:00Z"});
    let compiled = engine.compile(&logic).unwrap();
    let result = engine.evaluate_owned(&compiled, json!({})).unwrap();
    assert_eq!(result, json!(true));

    // String datetime - 10 AM should not be nighttime
    let logic = json!({"is_night": "2022-07-06T10:00:00Z"});
    let compiled = engine.compile(&logic).unwrap();
    let result = engine.evaluate_owned(&compiled, json!({})).unwrap();
    assert_eq!(result, json!(false));
}

#[test]
fn test_is_night_with_variable() {
    let mut engine = DataLogic::new();
    engine.add_operator("is_night".to_string(), Box::new(IsNightOperator));

    // Variable with nighttime
    let logic = json!({"is_night": {"var": "check_time"}});
    let compiled = engine.compile(&logic).unwrap();
    let data = json!({"check_time": {"datetime": "2022-07-06T23:00:00Z"}});
    let result = engine.evaluate_owned(&compiled, data).unwrap();
    assert_eq!(result, json!(true));

    // Variable with daytime
    let data = json!({"check_time": {"datetime": "2022-07-06T14:00:00Z"}});
    let result = engine.evaluate_owned(&compiled, data).unwrap();
    assert_eq!(result, json!(false));
}

#[test]
fn test_is_night_with_timezone() {
    let mut engine = DataLogic::new();
    engine.add_operator("is_night".to_string(), Box::new(IsNightOperator));

    // 10 PM UTC+5 converts to 5 PM UTC (not nighttime)
    let logic = json!({"is_night": {"datetime": "2022-07-06T22:00:00+05:00"}});
    let compiled = engine.compile(&logic).unwrap();
    let result = engine.evaluate_owned(&compiled, json!({})).unwrap();
    assert_eq!(result, json!(false));

    // 3 AM UTC+5 converts to 10 PM previous day UTC (nighttime)
    let logic = json!({"is_night": {"datetime": "2022-07-07T03:00:00+05:00"}});
    let compiled = engine.compile(&logic).unwrap();
    let result = engine.evaluate_owned(&compiled, json!({})).unwrap();
    assert_eq!(result, json!(true));
}

#[test]
fn test_is_night_with_preserve_structure() {
    let mut engine = DataLogic::with_preserve_structure();
    engine.add_operator("is_night".to_string(), Box::new(IsNightOperator));

    // Conditional logic in structured object - nighttime
    let logic = json!({
        "get_the_garlic": {
            "if": [
                {"is_night": {"datetime": "2022-07-06T23:59:59Z"}},
                {"should_i": "yes"},
                {"should_i": "nah"}
            ]
        }
    });

    let compiled = engine.compile(&logic).unwrap();
    let result = engine.evaluate_owned(&compiled, json!({})).unwrap();
    assert_eq!(result, json!({"get_the_garlic": {"should_i": "yes"}}));

    // Conditional logic in structured object - daytime
    let logic = json!({
        "get_the_garlic": {
            "if": [
                {"is_night": {"datetime": "2022-07-06T14:00:00Z"}},
                {"should_i": "yes"},
                {"should_i": "nah"}
            ]
        }
    });

    let compiled = engine.compile(&logic).unwrap();
    let result = engine.evaluate_owned(&compiled, json!({})).unwrap();
    assert_eq!(result, json!({"get_the_garlic": {"should_i": "nah"}}));
}

#[test]
fn test_is_night_error_invalid_argument() {
    let mut engine = DataLogic::new();
    engine.add_operator("is_night".to_string(), Box::new(IsNightOperator));

    // Invalid argument - number
    let logic = json!({"is_night": 42});
    let compiled = engine.compile(&logic).unwrap();
    let result = engine.evaluate_owned(&compiled, json!({}));
    assert!(result.is_err());

    // Invalid argument - invalid string
    let logic = json!({"is_night": "not a date"});
    let compiled = engine.compile(&logic).unwrap();
    let result = engine.evaluate_owned(&compiled, json!({}));
    assert!(result.is_err());
}

#[test]
fn test_is_night_error_argument_count() {
    let mut engine = DataLogic::new();
    engine.add_operator("is_night".to_string(), Box::new(IsNightOperator));

    // Missing argument
    let logic = json!({"is_night": []});
    let compiled = engine.compile(&logic).unwrap();
    let result = engine.evaluate_owned(&compiled, json!({}));
    assert!(result.is_err());

    // Too many arguments
    let logic = json!({"is_night": [
        {"datetime": "2022-07-06T20:00:00Z"},
        {"datetime": "2022-07-07T20:00:00Z"}
    ]});
    let compiled = engine.compile(&logic).unwrap();
    let result = engine.evaluate_owned(&compiled, json!({}));
    assert!(result.is_err());
}

#[test]
fn test_is_night_complex_structured_object() {
    let mut engine = DataLogic::with_preserve_structure();
    engine.add_operator("is_night".to_string(), Box::new(IsNightOperator));

    // Complex object with multiple custom operator uses
    let logic = json!({
        "vampire_status": {
            "active": {"is_night": {"datetime": "2022-07-06T22:00:00Z"}},
            "location": {"var": "location"},
            "threat_level": {
                "if": [
                    {"is_night": {"datetime": "2022-07-06T22:00:00Z"}},
                    "HIGH",
                    "LOW"
                ]
            }
        }
    });

    let compiled = engine.compile(&logic).unwrap();
    let data = json!({"location": "Transylvania"});
    let result = engine.evaluate_owned(&compiled, data).unwrap();

    assert_eq!(
        result,
        json!({
            "vampire_status": {
                "active": true,
                "location": "Transylvania",
                "threat_level": "HIGH"
            }
        })
    );
}
