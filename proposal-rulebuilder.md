## Description

Implement a fluent Rule Builder API that leverages arena allocation to efficiently create and optimize JSONLogic rules with minimal memory overhead and improved performance.

## Key Components

1. **Arena-Backed Builder**
   - Builder allocates all rule components directly in the memory arena
   - Zero-copy rule construction for optimal performance
   - Automatic memory management through the arena lifecycle

2. **Fluent Builder API**
   - Provides an intuitive, chainable interface for rule construction
   - Type-safe alternative to string-based or JSON-based rule creation
   - Eliminates the need to manually construct JSON objects for rules

3. **Factory Methods**
   - Provides factory methods for common rule patterns
   - Simplifies creation of complex nested rules
   - Reduces boilerplate for frequently used rule structures

## Example Usage

```rust
use datalogic_rs::{JsonLogic, RuleBuilder};
use serde_json::json;

// Create JSONLogic instance with arena
let logic = JsonLogic::new();

// Get a builder that uses the arena
let builder = logic.builder();

// Build a rule using the fluent API (all allocations happen in the arena)
let rule = builder
    .compare()
    .greater_than()
    .var("score")
    .value(50)
    .build();

// Evaluate the rule
let data = json!({"score": 75});
let result = logic.apply(&rule, &data).unwrap();
assert_eq!(result, json!(true));

// Create more complex rules with efficient memory usage
let filter_rule = builder
    .filter()
    .array(builder.var("users"))
    .condition(
        builder
            .compare()
            .greater_than_or_equal()
            .var("age")
            .value(30)
            .build()
    )
    .build();
```

## Technical Implementation

1. **Arena Integration**
   - Builder holds a reference to the JsonLogic's arena
   - All rule components are allocated directly in the arena
   - No intermediate allocations or copies during rule construction
   - Rules have lifetimes tied to the arena for memory safety

2. **Memory Efficiency**
   - Shared subexpressions can be reused without duplication
   - String literals are interned in the arena
   - Common values are cached to avoid redundant allocations
   - Rule structure is optimized during construction

3. **Performance Optimizations**
   - Static rule components are pre-evaluated during construction
   - Path expressions are pre-compiled for faster variable access
   - Rule structure is optimized for evaluation efficiency
   - Metadata is attached to rules to guide the evaluator

## Benefits

1. **Performance**
   - Direct rule creation without JSON parsing overhead
   - Optimal memory usage through arena allocation
   - Reduced allocations and improved cache locality
   - Potential for compile-time and construction-time optimizations

2. **Developer Experience**
   - More intuitive rule creation
   - Compile-time type checking for rule structure
   - Better IDE support with autocompletion
   - Clear error messages for invalid rule structures

3. **Code Readability**
   - Self-documenting rule creation
   - Clear structure for complex rules
   - Reduced nesting compared to JSON-based creation
   - Explicit intent through builder methods

## Implementation Scope

- Core builder interface for all JSONLogic operators
- Arena-based allocation for all rule components
- Factory methods for common patterns
- Rule optimization during construction
- Comprehensive documentation and examples
- Unit tests for all builder components
- Performance benchmarks comparing with JSON-based creation

## Dependencies

- Requires the arena allocation system to be implemented first
- Will be part of the v3.0 API redesign