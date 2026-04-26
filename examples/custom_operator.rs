//! Example demonstrating how to create and use custom operators in DataLogic.
//!
//! Custom operators are implemented via the `ArenaOperator` trait, which
//! receives **pre-evaluated** arguments as `&ArenaValue` borrows and returns
//! an arena-allocated `ArenaValue` result. This avoids the per-call clone of
//! `serde_json::Value` and is required to register a custom op with the
//! engine.

use bumpalo::Bump;
use datalogic_rs::{ArenaContextStack, ArenaOperator, ArenaValue, DataLogic, Error, Result};
use serde_json::json;

/// A simple operator that calculates the average of an array of numbers.
///
/// Usage: {"avg": [1, 2, 3, 4, 5]} -> 3
/// Or:    {"avg": {"var": "scores"}} -> average of scores array
struct AverageOperator;

impl ArenaOperator for AverageOperator {
    fn evaluate_arena<'a>(
        &self,
        args: &[&'a ArenaValue<'a>],
        _actx: &mut ArenaContextStack<'a>,
        arena: &'a Bump,
    ) -> Result<&'a ArenaValue<'a>> {
        if args.is_empty() {
            return Ok(arena.alloc(ArenaValue::Null));
        }

        // Collect numbers from each argument. Arrays unpack into their
        // numeric elements; primitive numbers are taken as-is.
        let mut numbers: Vec<f64> = Vec::new();
        for av in args {
            match av {
                ArenaValue::Array(items) => {
                    for it in items.iter() {
                        if let Some(n) = it.as_f64() {
                            numbers.push(n);
                        }
                    }
                }
                other => {
                    if let Some(n) = other.as_f64() {
                        numbers.push(n);
                    }
                }
            }
        }

        if numbers.is_empty() {
            return Ok(arena.alloc(ArenaValue::Null));
        }

        let avg = numbers.iter().sum::<f64>() / numbers.len() as f64;
        Ok(arena.alloc(ArenaValue::from_f64(avg)))
    }
}

/// An operator that checks if a value is within a range (inclusive).
///
/// Usage: {"between": [value, min, max]} -> boolean
struct BetweenOperator;

impl ArenaOperator for BetweenOperator {
    fn evaluate_arena<'a>(
        &self,
        args: &[&'a ArenaValue<'a>],
        _actx: &mut ArenaContextStack<'a>,
        arena: &'a Bump,
    ) -> Result<&'a ArenaValue<'a>> {
        if args.len() < 3 {
            return Err(Error::InvalidArguments(
                "between requires 3 arguments: value, min, max".to_string(),
            ));
        }
        let v = args[0]
            .as_f64()
            .ok_or_else(|| Error::InvalidArguments("value must be a number".into()))?;
        let lo = args[1]
            .as_f64()
            .ok_or_else(|| Error::InvalidArguments("min must be a number".into()))?;
        let hi = args[2]
            .as_f64()
            .ok_or_else(|| Error::InvalidArguments("max must be a number".into()))?;
        Ok(arena.alloc(ArenaValue::Bool(v >= lo && v <= hi)))
    }
}

/// An operator that formats a string with placeholders.
///
/// Usage: {"format": ["Hello, {}!", "World"]} -> "Hello, World!"
struct FormatOperator;

impl ArenaOperator for FormatOperator {
    fn evaluate_arena<'a>(
        &self,
        args: &[&'a ArenaValue<'a>],
        _actx: &mut ArenaContextStack<'a>,
        arena: &'a Bump,
    ) -> Result<&'a ArenaValue<'a>> {
        if args.is_empty() {
            return Err(Error::InvalidArguments(
                "format requires at least a template string".to_string(),
            ));
        }
        let template = args[0].as_str().ok_or_else(|| {
            Error::InvalidArguments("first argument must be a string".to_string())
        })?;

        let mut result = template.to_string();
        for av in args.iter().skip(1) {
            if let Some(pos) = result.find("{}") {
                let replacement = match av {
                    ArenaValue::String(s) => (*s).to_string(),
                    ArenaValue::Bool(b) => b.to_string(),
                    ArenaValue::Null => "null".to_string(),
                    ArenaValue::Number(_) => av
                        .as_f64()
                        .map(|n| {
                            if n.fract() == 0.0 {
                                (n as i64).to_string()
                            } else {
                                n.to_string()
                            }
                        })
                        .unwrap_or_default(),
                    _ => "<value>".to_string(),
                };
                result.replace_range(pos..pos + 2, &replacement);
            }
        }

        let s = arena.alloc_str(&result);
        Ok(arena.alloc(ArenaValue::String(s)))
    }
}

fn main() {
    println!("Custom Operator Examples\n");
    println!("========================\n");

    // Create engine and register custom operators
    let mut engine = DataLogic::new();
    engine.add_arena_operator("avg".to_string(), Box::new(AverageOperator));
    engine.add_arena_operator("between".to_string(), Box::new(BetweenOperator));
    engine.add_arena_operator("format".to_string(), Box::new(FormatOperator));

    // Example 1: Average operator
    println!("1. Average Operator");
    println!("-------------------");

    let logic = json!({"avg": [10, 20, 30, 40, 50]});
    let compiled = engine.compile(&logic).unwrap();
    let result = engine.evaluate_owned(&compiled, json!({})).unwrap();
    println!("   avg([10, 20, 30, 40, 50]) = {}", result);

    let logic = json!({"avg": {"var": "scores"}});
    let compiled = engine.compile(&logic).unwrap();
    let data = json!({"scores": [85, 90, 78, 92, 88]});
    let result = engine.evaluate_owned(&compiled, data).unwrap();
    println!("   avg(scores) = {} (from data)\n", result);

    // Example 2: Between operator
    println!("2. Between Operator");
    println!("-------------------");

    let logic = json!({"between": [{"var": "age"}, 18, 65]});
    let compiled = engine.compile(&logic).unwrap();

    let data1 = json!({"age": 25});
    let result1 = engine.evaluate_owned(&compiled, data1).unwrap();
    println!("   age=25 between 18 and 65? {}", result1);

    let data2 = json!({"age": 70});
    let result2 = engine.evaluate_owned(&compiled, data2).unwrap();
    println!("   age=70 between 18 and 65? {}\n", result2);

    // Example 3: Format operator
    println!("3. Format Operator");
    println!("------------------");

    let logic =
        json!({"format": ["Hello, {}! You have {} messages.", {"var": "name"}, {"var": "count"}]});
    let compiled = engine.compile(&logic).unwrap();
    let data = json!({"name": "Alice", "count": 5});
    let result = engine.evaluate_owned(&compiled, data).unwrap();
    println!("   {}\n", result);

    // Example 4: Combining custom with built-in operators
    println!("4. Combining Custom and Built-in Operators");
    println!("-------------------------------------------");

    let logic = json!({
        "if": [
            {"between": [{"var": "score"}, 90, 100]},
            "A",
            {"if": [
                {"between": [{"var": "score"}, 80, 89]},
                "B",
                {"if": [
                    {"between": [{"var": "score"}, 70, 79]},
                    "C",
                    "F"
                ]}
            ]}
        ]
    });
    let compiled = engine.compile(&logic).unwrap();

    for score in [95, 82, 75, 55] {
        let data = json!({"score": score});
        let result = engine.evaluate_owned(&compiled, data).unwrap();
        println!("   Score {} -> Grade {}", score, result);
    }

    println!("\nDone!");
}
