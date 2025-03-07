# Feature Request: Arena Allocation for Improved Performance

## Overview

Implement arena allocation using the bumpalo crate to significantly improve performance for complex rule evaluation, especially in high-throughput scenarios.

## User-Facing Changes

1. **New Instance-Based API**
   - Replace static methods with instance methods for arena-based evaluation
   - Provide a JsonLogic instance that manages its own memory arena

2. **Performance Improvements**
   - Faster rule evaluation, especially for complex rules and large datasets
   - Reduced memory usage and fewer allocations during evaluation
   - Lower GC pressure in high-throughput scenarios

3. **New API Methods**
   - `JsonLogic::new()` - Create a new JSONLogic evaluator with an arena
   - `logic.prepare(&rule_value)` - Prepare a rule value for efficient evaluation
   - `logic.apply(&rule, &data)` - Evaluate a prepared rule against data

4. **Memory Management**
   - Automatic memory cleanup after each evaluation
   - Option to manually reset the arena for reuse in long-running applications

## Migration Examples

### Before (v2.x)

```rust
use datalogic_rs::{JsonLogic, Rule};
use serde_json::json;

// Parse rule
let rule = Rule::from_value(&json!({">": [{"var": "score"}, 50]})).unwrap();

// Evaluate rule
let data = json!({"score": 75});
let result = JsonLogic::apply(&rule, &data).unwrap();
assert_eq!(result, json!(true));
```

### After (v3.0)

```rust
use datalogic_rs::JsonLogic;
use serde_json::json;

// Create JSONLogic instance with arena
let logic = JsonLogic::new();

// Prepare rule once, evaluate multiple times
let data = json!({"score": 75});
let rule_value = json!({">": [{"var": "score"}, 50]});
let rule = logic.prepare(&rule_value).unwrap();
let result = logic.apply(&rule, &data).unwrap();
assert_eq!(result, json!(true));
```

## Benefits

1. **Performance**
   - Profiling show 10-25% of time is spent on smaller memory allocation throughout the evaluation recursion. 
   - Reduced memory allocations and improved cache locality
   - Lower GC pressure in high-throughput scenarios

2. **Simplicity**
   - More ergonomic API for common use cases
   - Clear separation between rule preparation and evaluation
   - Efficient for scenarios where the same rule is evaluated against different data

3. **Resource Efficiency**
   - Better memory utilization for complex rules
   - Reduced CPU usage for rule evaluation
   - Improved throughput for high-volume applications

## Backward Compatibility

- The static `JsonLogic::apply` method will remain available for backward compatibility
- Existing code will continue to work but won't benefit from arena allocation
- Migration to the new API is recommended for performance-critical applications

## Implementation Notes

- Uses the bumpalo crate for efficient arena allocation
- Maintains the same semantics and behavior as the current implementation
- All tests will be updated to verify correctness with arena allocation
