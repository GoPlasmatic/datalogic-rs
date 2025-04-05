# Custom Operators in DataLogic-RS

This implementation adds support for custom operators in the JSONLogic Rust library. Users can now register and use their own custom operators alongside the built-in ones.

## Features

1. A `CustomOperator` trait that allows users to implement custom logic
2. Registration methods on `DataLogic` to add custom operators
3. Runtime evaluation of custom operators in JSONLogic expressions
4. Simple API that works with owned values and the arena allocation system

## Example Usage

```rust
use datalogic_rs::{CustomOperator, DataLogic, DataValue};
use datalogic_rs::arena::DataArena;
use datalogic_rs::value::NumberValue;

// Define a custom operator that multiplies all numbers in the array
struct MultiplyAll;

impl CustomOperator for MultiplyAll {
    fn evaluate<'a>(&self, args: &'a [DataValue<'a>], arena: &'a DataArena) -> Result<&'a DataValue<'a>> {
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

        // Return the result allocated in the arena
        Ok(arena.alloc(DataValue::Number(NumberValue::from_f64(product))))
    }
}

fn main() {
    let mut dl = DataLogic::new();
    
    // Register custom operator
    dl.register_custom_operator("multiply_all", Box::new(MultiplyAll));
    
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

1. **Arena Allocation**: The CustomOperator implementation works with the DataLogic arena allocation system, ensuring efficient memory management for custom operations.

2. **Memory Management**: While the JSONLogic library uses arena allocation internally for performance, the custom operator interface makes this straightforward to use.

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
