//! Example demonstrating how to create and use custom operators in Engine.
//!
//! Custom operators are implemented via the [`CustomOperator`] trait, which
//! receives **pre-evaluated** arguments as `&DataValue` borrows and returns
//! an arena-allocated `DataValue` result. This avoids per-call boundary
//! conversion and is required to register a custom op with the engine.
//!
//! Uses the v5 string-based API ([`Engine::evaluate_str`] for one-shots,
//! [`Engine::session`] for compile-once-evaluate-many) — no `serde_json`
//! boundary, no `compat` feature required.

use bumpalo::Bump;
use datalogic_rs::operator::ContextStack;
use datalogic_rs::{CustomOperator, DataValue, Engine, Error, Result};

/// Calculates the average of an array of numbers.
///
/// Usage: `{"avg": [1, 2, 3, 4, 5]}` -> `3`
/// Or:    `{"avg": {"var": "scores"}}` -> average of `scores` array
struct AverageOperator;

impl CustomOperator for AverageOperator {
    fn evaluate<'a>(
        &self,
        args: &[&'a DataValue<'a>],
        _ctx: &mut ContextStack<'a>,
        arena: &'a Bump,
    ) -> Result<&'a DataValue<'a>> {
        if args.is_empty() {
            return Ok(arena.alloc(DataValue::Null));
        }

        let mut numbers: Vec<f64> = Vec::new();
        for av in args {
            match av {
                DataValue::Array(items) => {
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
            return Ok(arena.alloc(DataValue::Null));
        }

        let avg = numbers.iter().sum::<f64>() / numbers.len() as f64;
        Ok(arena.alloc(DataValue::from_f64(avg)))
    }
}

/// Checks if a value is within a range (inclusive).
///
/// Usage: `{"between": [value, min, max]}` -> boolean
struct BetweenOperator;

impl CustomOperator for BetweenOperator {
    fn evaluate<'a>(
        &self,
        args: &[&'a DataValue<'a>],
        _ctx: &mut ContextStack<'a>,
        arena: &'a Bump,
    ) -> Result<&'a DataValue<'a>> {
        if args.len() < 3 {
            return Err(Error::invalid_arguments(
                "between requires 3 arguments: value, min, max".to_string(),
            ));
        }
        let v = args[0]
            .as_f64()
            .ok_or_else(|| Error::invalid_arguments("value must be a number"))?;
        let lo = args[1]
            .as_f64()
            .ok_or_else(|| Error::invalid_arguments("min must be a number"))?;
        let hi = args[2]
            .as_f64()
            .ok_or_else(|| Error::invalid_arguments("max must be a number"))?;
        Ok(arena.alloc(DataValue::Bool(v >= lo && v <= hi)))
    }
}

/// Formats a string with `{}` placeholders.
///
/// Usage: `{"format": ["Hello, {}!", "World"]}` -> `"Hello, World!"`
struct FormatOperator;

impl CustomOperator for FormatOperator {
    fn evaluate<'a>(
        &self,
        args: &[&'a DataValue<'a>],
        _ctx: &mut ContextStack<'a>,
        arena: &'a Bump,
    ) -> Result<&'a DataValue<'a>> {
        if args.is_empty() {
            return Err(Error::invalid_arguments(
                "format requires at least a template string".to_string(),
            ));
        }
        let template = args[0].as_str().ok_or_else(|| {
            Error::invalid_arguments("first argument must be a string".to_string())
        })?;

        let mut result = template.to_string();
        for av in args.iter().skip(1) {
            if let Some(pos) = result.find("{}") {
                let replacement = match av {
                    DataValue::String(s) => (*s).to_string(),
                    DataValue::Bool(b) => b.to_string(),
                    DataValue::Null => "null".to_string(),
                    DataValue::Number(_) => av
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
        Ok(arena.alloc(DataValue::String(s)))
    }
}

fn main() {
    println!("Custom Operator Examples\n");
    println!("========================\n");

    let engine = Engine::builder()
        .add_operator("avg", AverageOperator)
        .add_operator("between", BetweenOperator)
        .add_operator("format", FormatOperator)
        .build();

    // Example 1: Average operator
    println!("1. Average Operator");
    println!("-------------------");

    let result = engine
        .evaluate_str(r#"{"avg": [10, 20, 30, 40, 50]}"#, "{}")
        .unwrap();
    println!("   avg([10, 20, 30, 40, 50]) = {}", result);

    let result = engine
        .evaluate_str(
            r#"{"avg": {"var": "scores"}}"#,
            r#"{"scores": [85, 90, 78, 92, 88]}"#,
        )
        .unwrap();
    println!("   avg(scores) = {} (from data)\n", result);

    // Example 2: Between operator
    println!("2. Between Operator");
    println!("-------------------");

    let result1 = engine
        .evaluate_str(r#"{"between": [{"var": "age"}, 18, 65]}"#, r#"{"age": 25}"#)
        .unwrap();
    println!("   age=25 between 18 and 65? {}", result1);

    let result2 = engine
        .evaluate_str(r#"{"between": [{"var": "age"}, 18, 65]}"#, r#"{"age": 70}"#)
        .unwrap();
    println!("   age=70 between 18 and 65? {}\n", result2);

    // Example 3: Format operator
    println!("3. Format Operator");
    println!("------------------");

    let result = engine
        .evaluate_str(
            r#"{"format": ["Hello, {}! You have {} messages.", {"var": "name"}, {"var": "count"}]}"#,
            r#"{"name": "Alice", "count": 5}"#,
        )
        .unwrap();
    println!("   {}\n", result);

    // Example 4: Combining custom + built-in operators — compile once,
    // evaluate many. `Session` reuses the eval arena across calls.
    println!("4. Combining Custom and Built-in Operators");
    println!("-------------------------------------------");

    let grading_rule = r#"{
        "if": [
            {"between": [{"var": "score"}, 90, 100]}, "A",
            {"if": [
                {"between": [{"var": "score"}, 80, 89]}, "B",
                {"if": [
                    {"between": [{"var": "score"}, 70, 79]}, "C",
                    "F"
                ]}
            ]}
        ]
    }"#;

    let compiled = engine.compile(grading_rule).unwrap();
    let mut session = engine.session();

    for score in [95, 82, 75, 55] {
        let data = format!(r#"{{"score": {}}}"#, score);
        let grade = session.evaluate_str(&compiled, &data).unwrap();
        println!("   Score {} -> Grade {}", score, grade);
    }

    println!("\nDone!");
}
