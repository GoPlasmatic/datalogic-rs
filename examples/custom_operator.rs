use datalogic_rs::{JsonLogic, Rule, CustomOperator};
use datalogic_rs::Error;
use serde_json::{json, Value};
use std::borrow::Cow;

struct PowerOperator;

impl CustomOperator for PowerOperator {
    fn name(&self) -> &str {
        "pow"
    }
    
    fn apply<'a>(&self, args: &[Value], _data: &'a Value) -> Result<Cow<'a, Value>, Error> {
        // Validate number of arguments
        if args.len() != 2 {
            return Err(Error::InvalidArguments(
                "pow operator requires exactly 2 arguments".into()
            ));
        }

        // Extract and coerce arguments to numbers
        let base = match &args[0] {
            Value::Number(n) => n.as_f64().unwrap_or(0.0),
            Value::String(s) => s.parse::<f64>().unwrap_or(0.0),
            Value::Bool(b) => if *b { 1.0 } else { 0.0 },
            Value::Null => 0.0,
            _ => return Err(Error::Type("Base must be coercible to number".into()))
        };

        let exponent = match &args[1] {
            Value::Number(n) => n.as_f64().unwrap_or(0.0),
            Value::String(s) => s.parse::<f64>().unwrap_or(0.0),
            Value::Bool(b) => if *b { 1.0 } else { 0.0 },
            Value::Null => 0.0,
            _ => return Err(Error::Type("Exponent must be coercible to number".into()))
        };

        // Calculate power and handle special cases
        let result = if base == 0.0 && exponent < 0.0 {
            return Err(Error::Custom("Cannot raise zero to negative power".into()));
        } else {
            base.powf(exponent)
        };

        Ok(Cow::Owned(json!(result)))
    }
}

fn main() -> Result<(), Error> {
    // Register the power operator globally
    JsonLogic::global().add_operator(PowerOperator)?;

    // Test cases
    let test_cases = vec![
        (json!({"pow": [2, 3]}), json!(8.0)),
        (json!({"pow": [3, 2]}), json!(9.0)),
        (json!({"pow": [2, 0.5]}), json!(1.4142135623730951)),
        (json!({"pow": ["2", "3"]}), json!(8.0)),  // String coercion
        (json!({"pow": [true, 2]}), json!(1.0)),   // Boolean coercion
        (json!({"pow": [null, 5]}), json!(0.0)),   // Null coercion
    ];

    for (rule_json, expected) in test_cases {
        let rule = Rule::from_value(&rule_json)?;
        let result = JsonLogic::apply(&rule, &json!({}))?;
        assert_eq!(result, expected);
        println!("{} = {}", rule_json, result);
    }

    // Test error cases
    let error_cases = vec![
        json!({"pow": []}),                // Too few arguments
        json!({"pow": [1]}),               // Too few arguments
        json!({"pow": [1, 2, 3]}),         // Too many arguments
        json!({"pow": [0, -1]}),           // Zero base with negative exponent
    ];

    for rule_json in error_cases {
        let rule = Rule::from_value(&rule_json)?;
        match JsonLogic::apply(&rule, &json!({})) {
            Ok(result) => println!("Unexpected success: {} = {}", rule_json, result),
            Err(e) => println!("Expected error for {}: {}", rule_json, e),
        }
    }

    // Complex example using variables
    let complex_rule = json!({
        "pow": [
            {"var": "base"},
            {"var": "exponent"}
        ]
    });
    let rule = Rule::from_value(&complex_rule)?;
    let data = json!({
        "base": 2,
        "exponent": 3
    });
    let result = JsonLogic::apply(&rule, &data)?;
    assert_eq!(result, json!(8.0));
    println!("Complex rule result: {} = {}", complex_rule, result);

    Ok(())
}