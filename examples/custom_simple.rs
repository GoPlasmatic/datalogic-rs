// Simple example demonstrating custom operators with simplified API

use datalogic_rs::value::NumberValue;
use datalogic_rs::{DataLogic, DataValue};
use std::error::Error;

// Simple custom operator that doubles a number
// Now accepts data context as second parameter
fn double<'r>(
    args: Vec<DataValue<'r>>,
    data: DataValue<'r>,
) -> std::result::Result<DataValue<'r>, String> {
    if args.is_empty() {
        // If no arguments are provided, try to use a "value" from the data context
        if let Some(obj) = data.as_object() {
            for (key, val) in obj {
                if *key == "value" && val.is_number() {
                    if let Some(n) = val.as_f64() {
                        return Ok(DataValue::Number(NumberValue::from_f64(n * 2.0)));
                    }
                }
            }
        }
        return Err(
            "double operator requires at least one argument or 'value' in data".to_string(),
        );
    }

    if let Some(n) = args[0].as_f64() {
        return Ok(DataValue::Number(NumberValue::from_f64(n * 2.0)));
    }

    Err("Argument must be a number".to_string())
}

// String operator example - converts a string to uppercase
// Now accepts data context as second parameter
fn to_uppercase<'r>(
    args: Vec<DataValue<'r>>,
    data: DataValue<'r>,
) -> std::result::Result<DataValue<'r>, String> {
    if args.is_empty() {
        // If no arguments are provided, try to use a "text" from the data context
        if let Some(obj) = data.as_object() {
            for (key, val) in obj {
                if *key == "text" && val.is_string() {
                    if let Some(s) = val.as_str() {
                        let upper = s.to_uppercase();
                        let upper_str = Box::leak(upper.into_boxed_str());
                        return Ok(DataValue::String(upper_str));
                    }
                }
            }
        }
        return Err("to_uppercase requires a string argument or 'text' in data".to_string());
    }

    if let Some(s) = args[0].as_str() {
        // Use Box::leak to create a static string
        let upper = s.to_uppercase();
        let upper_str = Box::leak(upper.into_boxed_str());
        return Ok(DataValue::String(upper_str));
    }

    Err("Argument must be a string".to_string())
}

// Boolean operator example - checks if a number is even
// Now accepts data context as second parameter
fn is_even<'r>(
    args: Vec<DataValue<'r>>,
    data: DataValue<'r>,
) -> std::result::Result<DataValue<'r>, String> {
    if args.is_empty() {
        // If no arguments are provided, try to use a "number" from the data context
        if let Some(obj) = data.as_object() {
            for (key, val) in obj {
                if *key == "number" && val.is_number() {
                    if let Some(n) = val.as_i64() {
                        return Ok(DataValue::Bool(n % 2 == 0));
                    }
                }
            }
        }
        return Err("is_even requires a number argument or 'number' in data".to_string());
    }

    if let Some(n) = args[0].as_i64() {
        return Ok(DataValue::Bool(n % 2 == 0));
    }

    Err("Argument must be a number".to_string())
}

fn main() -> std::result::Result<(), Box<dyn Error>> {
    // Create a DataLogic instance
    let mut dl = DataLogic::new();

    // Register our simple custom operators
    dl.register_simple_operator("double", double);
    dl.register_simple_operator("to_uppercase", to_uppercase);
    dl.register_simple_operator("is_even", is_even);

    // Example 1: Double a number
    let result = dl.evaluate_str(r#"{"double": 5}"#, r#"{}"#)?;
    println!("Double 5 = {result}"); // Should print: Double 5 = 10

    // Example 2: Use the custom operator with a variable
    let result = dl.evaluate_str(r#"{"double": {"var": "value"}}"#, r#"{"value": 7.5}"#)?;
    println!("Double 7.5 = {result}"); // Should print: Double 7.5 = 15

    // Example 3: Convert a string to uppercase
    let result = dl.evaluate_str(r#"{"to_uppercase": "hello world"}"#, r#"{}"#)?;
    println!("Uppercase 'hello world' = {result}"); // Should print: HELLO WORLD

    // Example 4: Check if a number is even
    let result = dl.evaluate_str(r#"{"is_even": 42}"#, r#"{}"#)?;
    println!("Is 42 even? {result}"); // Should print: true

    // Example 5: Check if a number is even with a variable
    let result = dl.evaluate_str(r#"{"is_even": {"var": "number"}}"#, r#"{"number": 7}"#)?;
    println!("Is 7 even? {result}"); // Should print: false

    // Example 6: Using an operator with empty args - falling back to data context
    let result = dl.evaluate_str(r#"{"double": []}"#, r#"{"value": 3}"#)?;
    println!("Double from context = {result}"); // Should print: Double from context = 6

    // Example 7: Using an operator with empty args - falling back to data context
    let result = dl.evaluate_str(
        r#"{"to_uppercase": []}"#,
        r#"{"text": "direct from context"}"#,
    )?;
    println!("Uppercase from context = {result}"); // Should print: DIRECT FROM CONTEXT

    Ok(())
}
