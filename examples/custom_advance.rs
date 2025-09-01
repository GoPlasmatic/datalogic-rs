use datalogic_rs::arena::DataArena;
use datalogic_rs::value::NumberValue;
use datalogic_rs::{CustomOperator, DataLogic, DataValue, EvalContext, Result};
use serde_json::json;
use std::fmt::Debug;

// Define a custom operator that multiplies all numbers in the array
#[derive(Debug)]
struct MultiplyAll;

impl CustomOperator for MultiplyAll {
    fn evaluate<'a>(
        &self,
        args: &'a [DataValue<'a>],
        _context: &EvalContext<'a>,
        arena: &'a DataArena,
    ) -> Result<&'a DataValue<'a>> {
        // Default to 1 if no arguments provided
        if args.is_empty() {
            return Ok(arena.alloc(DataValue::Number(NumberValue::from_i64(1))));
        }

        // Calculate product of all numeric values
        let mut product = 1.0;
        for arg in args {
            if let Some(n) = arg.as_f64() {
                product *= n;
            }
        }

        // Return the result
        Ok(arena.alloc(DataValue::Number(NumberValue::from_f64(product))))
    }
}

// Define a custom operator that returns the median of a set of numbers
#[derive(Debug)]
struct Median;

impl CustomOperator for Median {
    fn evaluate<'a>(
        &self,
        args: &'a [DataValue<'a>],
        _context: &EvalContext<'a>,
        arena: &'a DataArena,
    ) -> Result<&'a DataValue<'a>> {
        // Collect all numeric values
        let mut numbers = Vec::new();

        // Handle the case where a single array is passed
        if args.len() == 1 && args[0].is_array() {
            if let Some(items) = args[0].as_array() {
                for item in items {
                    if let Some(n) = item.as_f64() {
                        numbers.push(n);
                    }
                }
            }
        } else {
            // Handle the case where multiple arguments are passed
            for arg in args {
                if let Some(n) = arg.as_f64() {
                    numbers.push(n);
                }
            }
        }

        // Return 0 for empty arrays
        if numbers.is_empty() {
            return Ok(arena.alloc(DataValue::Number(NumberValue::from_i64(0))));
        }

        // Sort the numbers
        numbers.sort_by(|a, b| a.partial_cmp(b).unwrap());

        // Calculate the median
        let median = if numbers.len() % 2 == 0 {
            // Even number of elements - average the middle two
            let mid = numbers.len() / 2;
            (numbers[mid - 1] + numbers[mid]) / 2.0
        } else {
            // Odd number of elements - take the middle one
            numbers[numbers.len() / 2]
        };

        // Return the result
        Ok(arena.alloc(DataValue::Number(NumberValue::from_f64(median))))
    }
}

fn main() {
    let mut dl = DataLogic::new();

    // Register custom operators
    dl.register_custom_operator("multiply_all", Box::new(MultiplyAll));
    dl.register_custom_operator("median", Box::new(Median));

    // Example 1: Multiply numbers
    let result = dl
        .evaluate_str(r#"{"multiply_all": [2, 3, 4]}"#, r#"{}"#, None)
        .unwrap();

    println!("Product of [2, 3, 4] = {result}");

    // Example 2: Find median of a set of numbers
    let result = dl
        .evaluate_json(&json!({"median": [7, 3, 5, 9, 1]}), &json!({}), None)
        .unwrap();

    println!("Median of [7, 3, 5, 9, 1] = {result}");

    // Example 3: Use custom operators in a complex logic expression
    let complex_logic = json!({
        "if": [
            {"<": [{"var": "score"}, 60]},
            "Failed",
            {">=": [{"var": "score"}, {"median": {"var": "class_scores"}}]},
            "Above average",
            "Below average"
        ]
    });

    let data = json!({
        "score": 75,
        "class_scores": [60, 70, 80, 90, 100]
    });

    let result = dl.evaluate_json(&complex_logic, &data, None).unwrap();
    println!("Student result: {result}");

    // Example 4: Combine custom operators
    let combined_logic = json!({
        "multiply_all": [
            2,
            {"median": [1, 2, 3, 4, 5]},
            4
        ]
    });

    let result = dl.evaluate_json(&combined_logic, &json!({}), None).unwrap();
    println!("2 * median([1, 2, 3, 4, 5]) * 4 = {result}");
}
