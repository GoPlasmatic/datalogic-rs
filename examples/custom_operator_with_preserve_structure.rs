//! Example demonstrating custom operators in preserve_structure mode
//!
//! This example shows how custom operators are properly recognized and evaluated
//! when preserve_structure is enabled, allowing them to work seamlessly within
//! structured objects alongside built-in operators.

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

fn main() {
    println!("Custom Operators with Preserve Structure\n");

    // Create engine with preserve_structure enabled
    let mut engine = DataLogic::with_preserve_structure();
    engine.add_operator("is_night".to_string(), Box::new(IsNightOperator));

    println!("=== Conditional Logic with Custom Operators ===\n");

    // Nighttime check
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
    println!("Nighttime (11:59 PM): {}", result);

    // Daytime check
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
    println!("Daytime (2:00 PM):    {}\n", result);

    println!("=== Complex Structured Objects ===\n");

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
    println!("Mixed operators: {}", result);
}
