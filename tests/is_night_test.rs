//! Tests for arena custom operators against datetime / structured-object inputs.

use bumpalo::Bump;
use chrono::{DateTime, Timelike};
use datalogic_rs::{ArenaContextStack, ArenaOperator, ArenaValue, DataLogic, Error, Result};
use serde_json::{Value, json};

/// Custom arena operator: checks whether a datetime argument falls in the
/// nighttime window (hours outside 7..19 UTC). Demonstrates an `ArenaOperator`
/// that pulls a string out of a pre-evaluated arg without round-tripping
/// through `serde_json::Value`.
struct IsNightOperator;

impl ArenaOperator for IsNightOperator {
    fn evaluate_arena<'a>(
        &self,
        args: &[&'a ArenaValue<'a>],
        _actx: &mut ArenaContextStack<'a>,
        arena: &'a Bump,
    ) -> Result<&'a ArenaValue<'a>> {
        if args.len() != 1 {
            return Err(Error::InvalidArguments(
                "Expected exactly one argument".to_string(),
            ));
        }

        let dt = parse_datetime_arena(args[0])
            .ok_or_else(|| Error::InvalidArguments("Invalid datetime argument".to_string()))?;
        let hour = dt.hour();
        let is_night = !(7..19).contains(&hour);
        Ok(arena.alloc(ArenaValue::Bool(is_night)))
    }
}

fn parse_datetime_arena(av: &ArenaValue<'_>) -> Option<DateTime<chrono::Utc>> {
    // Arena-resident string (e.g., from a custom op chain or input).
    if let Some(s) = av.as_str()
        && let Ok(dt) = DateTime::parse_from_rfc3339(s)
    {
        return Some(dt.with_timezone(&chrono::Utc));
    }
    // Arena-resident object — walk it for a `datetime` field.
    if let ArenaValue::Object(pairs) = av {
        for (k, v) in pairs.iter() {
            if *k == "datetime"
                && let Some(s) = v.as_str()
                && let Ok(dt) = DateTime::parse_from_rfc3339(s)
            {
                return Some(dt.with_timezone(&chrono::Utc));
            }
        }
    }
    None
}

fn parse_datetime_value(value: &Value) -> Option<DateTime<chrono::Utc>> {
    if let Value::Object(map) = value
        && let Some(Value::String(datetime_str)) = map.get("datetime")
        && let Ok(dt) = DateTime::parse_from_rfc3339(datetime_str)
    {
        return Some(dt.with_timezone(&chrono::Utc));
    }
    if let Value::String(datetime_str) = value
        && let Ok(dt) = DateTime::parse_from_rfc3339(datetime_str)
    {
        return Some(dt.with_timezone(&chrono::Utc));
    }
    None
}

#[test]
fn test_is_night_nighttime() {
    let mut engine = DataLogic::new();
    engine.add_arena_operator("is_night".to_string(), Box::new(IsNightOperator));

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
    engine.add_arena_operator("is_night".to_string(), Box::new(IsNightOperator));

    let logic = json!({"is_night": {"datetime": "2022-07-06T08:00:00Z"}});
    let compiled = engine.compile(&logic).unwrap();
    let result = engine.evaluate_owned(&compiled, json!({})).unwrap();
    assert_eq!(result, json!(false));

    let logic = json!({"is_night": {"datetime": "2022-07-06T12:00:00Z"}});
    let compiled = engine.compile(&logic).unwrap();
    let result = engine.evaluate_owned(&compiled, json!({})).unwrap();
    assert_eq!(result, json!(false));

    let logic = json!({"is_night": {"datetime": "2022-07-06T15:00:00Z"}});
    let compiled = engine.compile(&logic).unwrap();
    let result = engine.evaluate_owned(&compiled, json!({})).unwrap();
    assert_eq!(result, json!(false));
}

#[test]
fn test_is_night_boundaries() {
    let mut engine = DataLogic::new();
    engine.add_arena_operator("is_night".to_string(), Box::new(IsNightOperator));

    let logic = json!({"is_night": {"datetime": "2022-07-06T19:00:00Z"}});
    let compiled = engine.compile(&logic).unwrap();
    let result = engine.evaluate_owned(&compiled, json!({})).unwrap();
    assert_eq!(result, json!(true));

    let logic = json!({"is_night": {"datetime": "2022-07-06T07:00:00Z"}});
    let compiled = engine.compile(&logic).unwrap();
    let result = engine.evaluate_owned(&compiled, json!({})).unwrap();
    assert_eq!(result, json!(false));

    let logic = json!({"is_night": {"datetime": "2022-07-06T06:59:59Z"}});
    let compiled = engine.compile(&logic).unwrap();
    let result = engine.evaluate_owned(&compiled, json!({})).unwrap();
    assert_eq!(result, json!(true));
}

#[test]
fn test_is_night_with_string() {
    let mut engine = DataLogic::new();
    engine.add_arena_operator("is_night".to_string(), Box::new(IsNightOperator));

    let logic = json!({"is_night": "2022-07-06T21:00:00Z"});
    let compiled = engine.compile(&logic).unwrap();
    let result = engine.evaluate_owned(&compiled, json!({})).unwrap();
    assert_eq!(result, json!(true));

    let logic = json!({"is_night": "2022-07-06T10:00:00Z"});
    let compiled = engine.compile(&logic).unwrap();
    let result = engine.evaluate_owned(&compiled, json!({})).unwrap();
    assert_eq!(result, json!(false));
}

#[test]
fn test_is_night_with_variable() {
    let mut engine = DataLogic::new();
    engine.add_arena_operator("is_night".to_string(), Box::new(IsNightOperator));

    let logic = json!({"is_night": {"var": "check_time"}});
    let compiled = engine.compile(&logic).unwrap();
    let data = json!({"check_time": {"datetime": "2022-07-06T23:00:00Z"}});
    let result = engine.evaluate_owned(&compiled, data).unwrap();
    assert_eq!(result, json!(true));

    let data = json!({"check_time": {"datetime": "2022-07-06T14:00:00Z"}});
    let result = engine.evaluate_owned(&compiled, data).unwrap();
    assert_eq!(result, json!(false));
}

#[test]
fn test_is_night_with_timezone() {
    let mut engine = DataLogic::new();
    engine.add_arena_operator("is_night".to_string(), Box::new(IsNightOperator));

    let logic = json!({"is_night": {"datetime": "2022-07-06T22:00:00+05:00"}});
    let compiled = engine.compile(&logic).unwrap();
    let result = engine.evaluate_owned(&compiled, json!({})).unwrap();
    assert_eq!(result, json!(false));

    let logic = json!({"is_night": {"datetime": "2022-07-07T03:00:00+05:00"}});
    let compiled = engine.compile(&logic).unwrap();
    let result = engine.evaluate_owned(&compiled, json!({})).unwrap();
    assert_eq!(result, json!(true));
}

#[test]
fn test_is_night_with_preserve_structure() {
    let mut engine = DataLogic::with_preserve_structure();
    engine.add_arena_operator("is_night".to_string(), Box::new(IsNightOperator));

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
    engine.add_arena_operator("is_night".to_string(), Box::new(IsNightOperator));

    let logic = json!({"is_night": 42});
    let compiled = engine.compile(&logic).unwrap();
    let result = engine.evaluate_owned(&compiled, json!({}));
    assert!(result.is_err());

    let logic = json!({"is_night": "not a date"});
    let compiled = engine.compile(&logic).unwrap();
    let result = engine.evaluate_owned(&compiled, json!({}));
    assert!(result.is_err());
}

#[test]
fn test_is_night_error_argument_count() {
    let mut engine = DataLogic::new();
    engine.add_arena_operator("is_night".to_string(), Box::new(IsNightOperator));

    let logic = json!({"is_night": []});
    let compiled = engine.compile(&logic).unwrap();
    let result = engine.evaluate_owned(&compiled, json!({}));
    assert!(result.is_err());

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
    engine.add_arena_operator("is_night".to_string(), Box::new(IsNightOperator));

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
