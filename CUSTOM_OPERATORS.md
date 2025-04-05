# Custom Operators in DataLogic-RS

This implementation adds support for custom operators in the JSONLogic Rust library. Users can now register and use their own custom operators alongside the built-in ones.

## Features

1. A `CustomOperator` trait that allows users to implement custom logic
2. Registration methods on `DataLogic` to add custom operators
3. Runtime evaluation of custom operators in JSONLogic expressions
4. Simple API that works with owned values (no arena complexity exposed)

## Example Usage

```rust
use datalogic_rs::{CustomOperator, DataLogic, DataValue};

// Define a custom operator that multiplies all numbers in the array
struct MultiplyAll;

impl CustomOperator for MultiplyAll {
    fn evaluate(&self, args: &[DataValue]) -> Result<DataValue, String> {
        // Default to 1 if no arguments provided
        if args.is_empty() {
            return Ok(DataValue::from(1));
        }

        // Calculate product of all numeric values
        let mut product = 1.0;
        for arg in args {
            if let Some(n) = arg.as_f64() {
                product *= n;
            }
        }

        // Return the result
        Ok(DataValue::from(product))
    }
}

fn main() {
    let mut dl = DataLogic::new();
    
    // Register custom operator
    dl.register_operator("multiply_all", MultiplyAll);
    
    // Use the custom operator
    let result = dl.evaluate_str(
        r#"{"multiply_all": [2, 3, 4]}"#,
        r#"{}"#,
        None
    ).unwrap();
    
    println!("Product: {}", result); // Output: 24
}
```

## Design Considerations

1. **Simple API**: The API is designed to be simple and intuitive for users. Custom operators work with owned values rather than arena references.

2. **Memory Management**: While the JSONLogic library uses arena allocation internally for performance, the custom operator interface abstracts this away, allowing users to focus on their logic.

3. **Composition**: Custom operators can be combined with built-in operators, allowing for complex expressions.

## Advanced Usage 

See the `examples/custom.rs` file for more advanced examples, including:

1. Using custom operators within complex logic expressions
2. Combining multiple custom operators
3. Working with arrays and objects in custom operators

For example, you can define a "median" custom operator and use it within an "if" expression:

```rust
let complex_logic = r#"{
    "if": [
        {"<": [{"var": "score"}, 60]},
        "Failed",
        {">=": [{"var": "score"}, {"median": {"var": "class_scores"}}]},
        "Above average",
        "Below average"
    ]
}"#;

// This uses the "median" custom operator within an "if" expression
```
