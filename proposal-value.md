## Description

Redesign the core value representation in JSONLogic to replace direct dependency on `serde_json::Value` with a custom, memory-efficient value type that leverages arena allocation. This will significantly reduce memory overhead, improve performance, and enable more efficient rule evaluation while maintaining compatibility with the existing API.

## Key Components

1. **Custom Value Type**
   - Implement a custom `JsonValue` type optimized for JSONLogic operations
   - Design for minimal memory footprint and efficient arena allocation
   - Support zero-copy operations where possible

2. **Arena-Based Allocation**
   - Implement arena allocation for all value types
   - Enable shared string storage with string interning
   - Support for value reuse and reference counting

3. **Efficient Serialization/Deserialization**
   - Maintain compatibility with serde_json for input/output
   - Optimize conversion between custom value type and serde_json
   - Support direct parsing from string to custom value type

## Proposed Implementation

```rust
// Custom value type optimized for JSONLogic
pub enum JsonValue<'a> {
    Null,
    Bool(bool),
    Number(NumberValue<'a>),
    String(&'a str),  // Arena-allocated string reference
    Array(ArrayValue<'a>),
    Object(ObjectValue<'a>),
}

// Number representation with specialized types
pub enum NumberValue<'a> {
    Integer(i64),
    Float(f64),
    // Optional: BigInt support for arbitrary precision
}

// Efficient array representation
pub struct ArrayValue<'a> {
    elements: &'a [JsonValue<'a>],  // Arena-allocated slice
}

// Efficient object representation
pub struct ObjectValue<'a> {
    entries: &'a [(KeyRef<'a>, JsonValue<'a>)],  // Arena-allocated entries
}

// String interning for keys
pub struct KeyRef<'a>(&'a str);  // Interned string reference

// Arena for value allocation
pub struct JsonArena {
    // Implementation details for memory management
}

// Updated JsonLogic structure
pub struct JsonLogic {
    // Existing fields
    custom_operators: RwLock<HashMap<String, CustomOperatorBox>>,
    // New fields
    arena: JsonArena,
}
```

## API Changes

```rust
impl JsonLogic {
    // New instance constructor with arena
    pub fn new() -> Self {
        Self {
            custom_operators: RwLock::new(HashMap::new()),
            arena: JsonArena::new(),
        }
    }
    
    // New method to prepare a rule with arena allocation
    pub fn prepare<'a>(&'a self, value: &Value) -> Result<ArenaRule<'a>, Error> {
        // Parse serde_json::Value into arena-allocated rule
    }
    
    // New method to prepare a rule from string with arena allocation
    pub fn prepare_str<'a>(&'a self, json_str: &str) -> Result<ArenaRule<'a>, Error> {
        // Parse JSON string directly into arena-allocated rule
    }
    
    // New method to apply arena-allocated rule
    pub fn apply<'a>(&'a self, rule: &ArenaRule<'a>, data: &Value) -> JsonLogicResult {
        // Convert data to arena-allocated value and evaluate
    }
    
    // Existing static method (unchanged for backward compatibility)
    pub fn apply_static(rule: &Rule, data: &Value) -> JsonLogicResult {
        // Existing implementation
    }
}
```

## Example Usage

```rust
use datalogic_rs::JsonLogic;
use serde_json::json;

// Create instance with arena
let logic = JsonLogic::new();

// Prepare rule with arena allocation
let rule_value = json!({">": [{"var": "score"}, 50]});
let rule = logic.prepare(&rule_value).unwrap();

// Apply rule with arena allocation
let data = json!({"score": 75});
let result = logic.apply(&rule, &data).unwrap();
assert_eq!(result, json!(true));

// Or prepare rule directly from string
let rule_str = r#"{">": [{"var": "score"}, 50]}"#;
let rule = logic.prepare_str(rule_str).unwrap();
let result = logic.apply(&rule, &data).unwrap();
```

## Technical Implementation

1. **Custom Value Type**
   - Implement a memory-efficient value type that minimizes allocations
   - Design for optimal memory layout and cache efficiency
   - Support for zero-copy operations where possible

2. **Arena Allocation System**
   - Implement a bump allocator for efficient memory management
   - Support for string interning to deduplicate strings
   - Enable value sharing and reference counting

3. **Efficient Parsing**
   - Implement direct parsing from JSON string to arena-allocated values
   - Optimize conversion between serde_json::Value and custom value type
   - Support for incremental parsing for large documents

4. **Backward Compatibility Layer**
   - Maintain existing API for backward compatibility
   - Provide migration path for users to adopt arena-based API
   - Document performance benefits and migration strategies

## Benefits

1. **Reduced Memory Usage**
   - Minimize allocations through arena-based memory management
   - Eliminate redundant string storage through string interning
   - Share common substructures across rules and data

2. **Improved Performance**
   - Reduce allocation overhead during rule evaluation
   - Optimize memory layout for better cache efficiency
   - Enable zero-copy operations for common patterns

3. **Enhanced Scalability**
   - Support for larger rule sets and data structures
   - Reduce garbage collection pressure in high-throughput scenarios
   - Enable more efficient parallel processing

## Implementation Scope

- Implement custom value type optimized for JSONLogic
- Develop arena allocation system for efficient memory management
- Create direct parsing from JSON string to arena-allocated values
- Maintain backward compatibility with existing API
- Add comprehensive documentation and migration guides
- Develop benchmarks to measure performance improvements
