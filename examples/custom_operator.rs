//! Example demonstrating how to create and use custom operators in DataLogic.
//!
//! Custom operators allow you to extend JSONLogic with domain-specific functionality.
//! This example shows several patterns for implementing custom operators.
//!
//! IMPORTANT: Custom operators receive UNEVALUATED arguments (raw JSON logic).
//! You must call `evaluator.evaluate(&arg, context)` to evaluate each argument.

use datalogic_rs::{ContextStack, DataLogic, Error, Evaluator, Operator, Result};
use serde_json::{Value, json};

/// A simple operator that calculates the average of an array of numbers.
///
/// Usage: {"avg": [1, 2, 3, 4, 5]} -> 3
/// Or:    {"avg": {"var": "scores"}} -> average of scores array
struct AverageOperator;

impl Operator for AverageOperator {
    fn evaluate(
        &self,
        args: &[Value],
        context: &mut ContextStack,
        evaluator: &dyn Evaluator,
    ) -> Result<Value> {
        if args.is_empty() {
            return Ok(Value::Null);
        }

        // Evaluate the first argument (it could be a var reference, literal, etc.)
        let evaluated = evaluator.evaluate(&args[0], context)?;

        // Collect numbers from the evaluated result
        let numbers: Vec<f64> = if let Some(arr) = evaluated.as_array() {
            arr.iter().filter_map(|v| v.as_f64()).collect()
        } else if let Some(n) = evaluated.as_f64() {
            // Single number - evaluate all args and average them
            let mut nums = vec![n];
            for arg in args.iter().skip(1) {
                let val = evaluator.evaluate(arg, context)?;
                if let Some(n) = val.as_f64() {
                    nums.push(n);
                }
            }
            nums
        } else {
            return Ok(Value::Null);
        };

        if numbers.is_empty() {
            return Ok(Value::Null);
        }

        let sum: f64 = numbers.iter().sum();
        let avg = sum / numbers.len() as f64;
        Ok(json!(avg))
    }
}

/// An operator that checks if a value is within a range (inclusive).
///
/// Usage: {"between": [value, min, max]} -> boolean
struct BetweenOperator;

impl Operator for BetweenOperator {
    fn evaluate(
        &self,
        args: &[Value],
        context: &mut ContextStack,
        evaluator: &dyn Evaluator,
    ) -> Result<Value> {
        if args.len() < 3 {
            return Err(Error::InvalidArguments(
                "between requires 3 arguments: value, min, max".to_string(),
            ));
        }

        // Evaluate all arguments first
        let value_result = evaluator.evaluate(&args[0], context)?;
        let min_result = evaluator.evaluate(&args[1], context)?;
        let max_result = evaluator.evaluate(&args[2], context)?;

        let value = value_result.as_f64().ok_or_else(|| {
            Error::InvalidArguments("First argument must evaluate to a number".to_string())
        })?;
        let min = min_result.as_f64().ok_or_else(|| {
            Error::InvalidArguments("Second argument must evaluate to a number".to_string())
        })?;
        let max = max_result.as_f64().ok_or_else(|| {
            Error::InvalidArguments("Third argument must evaluate to a number".to_string())
        })?;

        Ok(json!(value >= min && value <= max))
    }
}

/// An operator that formats a string with placeholders.
///
/// Usage: {"format": ["Hello, {}!", "World"]} -> "Hello, World!"
/// Or:    {"format": ["Hello, {}!", {"var": "name"}]} -> "Hello, Alice!"
struct FormatOperator;

impl Operator for FormatOperator {
    fn evaluate(
        &self,
        args: &[Value],
        context: &mut ContextStack,
        evaluator: &dyn Evaluator,
    ) -> Result<Value> {
        if args.is_empty() {
            return Err(Error::InvalidArguments(
                "format requires at least a template string".to_string(),
            ));
        }

        // Evaluate the template
        let template_val = evaluator.evaluate(&args[0], context)?;
        let template = template_val.as_str().ok_or_else(|| {
            Error::InvalidArguments("First argument must evaluate to a string".to_string())
        })?;

        let mut result = template.to_string();
        for arg in args.iter().skip(1) {
            if let Some(pos) = result.find("{}") {
                // Evaluate each replacement argument
                let evaluated = evaluator.evaluate(arg, context)?;
                let replacement = match &evaluated {
                    Value::String(s) => s.clone(),
                    Value::Number(n) => n.to_string(),
                    Value::Bool(b) => b.to_string(),
                    Value::Null => "null".to_string(),
                    _ => evaluated.to_string(),
                };
                result.replace_range(pos..pos + 2, &replacement);
            }
        }

        Ok(json!(result))
    }
}

/// An operator that evaluates sub-expressions conditionally (demonstrates recursion).
///
/// Usage: {"when_positive": [{"var": "x"}, {"*": [{"var": "x"}, 2]}]}
/// Returns the second expression's result only if first expression is positive, otherwise null.
struct WhenPositiveOperator;

impl Operator for WhenPositiveOperator {
    fn evaluate(
        &self,
        args: &[Value],
        context: &mut ContextStack,
        evaluator: &dyn Evaluator,
    ) -> Result<Value> {
        if args.len() < 2 {
            return Err(Error::InvalidArguments(
                "when_positive requires 2 arguments".to_string(),
            ));
        }

        // Evaluate the first argument to get the condition value
        let condition = evaluator.evaluate(&args[0], context)?;
        let value = condition.as_f64();

        if let Some(v) = value {
            if v > 0.0 {
                // Recursively evaluate the second argument
                return evaluator.evaluate(&args[1], context);
            }
        }

        Ok(Value::Null)
    }
}

fn main() {
    println!("Custom Operator Examples\n");
    println!("========================\n");

    // Create engine and register custom operators
    let mut engine = DataLogic::new();
    engine.add_operator("avg".to_string(), Box::new(AverageOperator));
    engine.add_operator("between".to_string(), Box::new(BetweenOperator));
    engine.add_operator("format".to_string(), Box::new(FormatOperator));
    engine.add_operator("when_positive".to_string(), Box::new(WhenPositiveOperator));

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

    println!("\n5. When Positive Operator");
    println!("-------------------------");

    let logic = json!({"when_positive": [{"var": "x"}, {"*": [{"var": "x"}, 2]}]});
    let compiled = engine.compile(&logic).unwrap();

    let data1 = json!({"x": 5});
    let result1 = engine.evaluate_owned(&compiled, data1).unwrap();
    println!("   x=5 -> {}", result1);

    let data2 = json!({"x": -3});
    let result2 = engine.evaluate_owned(&compiled, data2).unwrap();
    println!("   x=-3 -> {}", result2);

    println!("\nDone!");
}
