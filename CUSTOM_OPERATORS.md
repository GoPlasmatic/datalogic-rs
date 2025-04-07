# Custom Operators in DataLogic-RS

DataLogic-RS offers two approaches for implementing custom operators, each with different trade-offs:

1. **CustomSimple** - A simplified API that works with owned values, ideal for scalar operations
2. **CustomAdvanced** - Direct arena access for maximum performance and complex data structures

## CustomSimple Operators

CustomSimple operators provide an easy way to implement custom logic without having to deal with arena allocation or complex lifetime management.

### Implementing a CustomSimple Operator

#### Step 1: Define the operator function

```rust
use datalogic_rs::{DataValue, SimpleOperatorFn};
use datalogic_rs::value::NumberValue;

// Simple custom operator that doubles a number
fn double<'r>(args: Vec<DataValue<'r>>, data: DataValue<'r>) -> std::result::Result<DataValue<'r>, String> {
    if args.is_empty() {
        // If no arguments provided, check if we can get a value from data context
        if let Some(obj) = data.as_object() {
            for (key, val) in obj {
                if *key == "value" && val.is_number() {
                    if let Some(n) = val.as_f64() {
                        return Ok(DataValue::Number(NumberValue::from_f64(n * 2.0)));
                    }
                }
            }
        }
        return Err("double operator requires at least one argument or 'value' in data".to_string());
    }
    
    if let Some(n) = args[0].as_f64() {
        return Ok(DataValue::Number(NumberValue::from_f64(n * 2.0)));
    }
    
    Err("Argument must be a number".to_string())
}

// String operator example
fn to_uppercase<'r>(args: Vec<DataValue<'r>>, data: DataValue<'r>) -> std::result::Result<DataValue<'r>, String> {
    if args.is_empty() {
        // If no arguments provided, check if we can get a text from data context
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

// Boolean operator example
fn is_even<'r>(args: Vec<DataValue<'r>>, data: DataValue<'r>) -> std::result::Result<DataValue<'r>, String> {
    if args.is_empty() {
        // If no arguments provided, check if we can get a number from data context
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
```

#### Step 2: Register the operator

```rust
use datalogic_rs::DataLogic;

let mut dl = DataLogic::new();
dl.register_simple_operator("double", double);
dl.register_simple_operator("to_uppercase", to_uppercase);
dl.register_simple_operator("is_even", is_even);
```

#### Step 3: Use the operator in rules

```rust
// Numeric operator
let result = dl.evaluate_str(
    r#"{"double": 5}"#,
    r#"{}"#,
    None
)?;
// result = 10

// String operator
let result = dl.evaluate_str(
    r#"{"to_uppercase": "hello"}"#,
    r#"{}"#,
    None
)?;
// result = "HELLO"

// Boolean operator
let result = dl.evaluate_str(
    r#"{"is_even": 4}"#,
    r#"{}"#,
    None
)?;
// result = true

// Using data context directly
let result = dl.evaluate_str(
    r#"{"double": []}"#,
    r#"{"value": 3}"#,
    None
)?;
// result = 6
```

### How CustomSimple Works

Behind the scenes, the CustomSimple API:

1. Converts arena-referenced `DataValue` instances to owned instances
2. Calls your function with these owned values and the current data context
3. Takes the returned owned value and allocates it back in the arena

The implementation is organized in a dedicated `src/arena/custom_operator.rs` module.

### Limitations of CustomSimple

The CustomSimple API has these limitations:

1. Does not support returning complex data types (arrays and objects)
2. Slightly higher overhead due to conversion between arena and owned values
3. Not suitable for operators that need to manipulate the structure of complex inputs

### Working with Owned Values

When you need to evaluate a rule with owned values, you can use the `to_owned_value` helper function:

```rust
use datalogic_rs::{DataLogic, DataValue, to_owned_value};

// Create some owned data to use in evaluation
let mut data = std::collections::HashMap::new();
data.insert("value".to_string(), 42);

// Convert owned data to DataValue format that can be used in evaluation
let data_value = to_owned_value(data);

// Use in evaluation
let result = dl.evaluate_with_data(
    r#"{"var": "value"}"#,
    &data_value
)?;
```

## CustomAdvanced Operators

CustomAdvanced operators provide direct access to the arena allocation system for maximum performance and flexibility.

### Implementing a CustomAdvanced Operator

```rust
use datalogic_rs::{CustomOperator, DataLogic, DataValue};
use datalogic_rs::arena::DataArena;
use datalogic_rs::value::NumberValue;
use datalogic_rs::logic::Result;

// Define a custom operator that multiplies all numbers in the array
#[derive(Debug)]
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

### Advanced Example: Working with Arrays

```rust
#[derive(Debug)]
struct FilterEven;

impl CustomOperator for FilterEven {
    fn evaluate<'a>(&self, args: &'a [DataValue<'a>], arena: &'a DataArena) -> Result<&'a DataValue<'a>> {
        if args.is_empty() {
            return Ok(arena.empty_array_value());
        }

        if let Some(arr) = args[0].as_array() {
            // Filter numbers that are even
            let filtered: Vec<&DataValue> = arr
                .iter()
                .filter(|v| {
                    if let Some(n) = v.as_i64() {
                        n % 2 == 0
                    } else {
                        false
                    }
                })
                .collect();

            // Allocate the filtered array in the arena
            Ok(arena.alloc_array(&filtered))
        } else {
            Ok(arena.empty_array_value())
        }
    }
}
```

## When to Use Which API

### Use CustomSimple when:
1. You're working with scalar values (numbers, strings, booleans)
2. You want a simpler implementation without arena management
3. You don't need to return complex data structures
4. You want to access the current data context directly

### Use CustomAdvanced when:
1. You need to return complex data types (arrays, objects)
2. You need maximum performance
3. You want direct control over memory allocation

## Design Considerations

1. **Arena Allocation**: The CustomAdvanced implementation works with the DataLogic arena allocation system directly, ensuring efficient memory management.

2. **Memory Management**: The CustomSimple API handles arena allocation automatically, making it easier to use but with some performance overhead.

3. **Composition**: Both types of custom operators can be combined with built-in operators, allowing for complex expressions.

4. **Data Context**: Both approaches provide access to the current data context, allowing operators to access data directly.

## Best Practices

1. Focus on implementing operators that work with well-defined input types
2. Return meaningful error messages when inputs are not of the expected type
3. Keep your custom operators simple and focused on specific tasks
4. For strings in CustomSimple, remember to use `Box::leak` to create 'static references
5. For complex operations, prefer the CustomAdvanced API
6. When using the data context, check for existence of expected fields and handle missing cases gracefully

## Examples

See these examples for complete implementations:
- [simple_custom_operator.rs](examples/simple_custom_operator.rs) - Using the CustomSimple API
- [custom.rs](examples/custom.rs) - Using the CustomAdvanced API
