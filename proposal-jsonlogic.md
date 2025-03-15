## Description

Redesign the JSONLogic structure to incorporate arena allocation while maintaining backward compatibility with existing code. The new design will support flexible rule creation from both JSON values and strings, providing an efficient path for users to migrate to the arena-based implementation.

## Key Components

1. **Dual-Mode Architecture**
   - Support both arena-based and traditional allocation strategies
   - Maintain backward compatibility with existing static methods
   - Provide clear migration path for performance-sensitive applications

2. **Flexible Rule Creation**
   - Support creating rules from both `serde_json::Value` and string sources
   - Unified API for different input formats
   - Optimized parsing paths for each input type

3. **Updated Core Structure**
   - Add arena field to JsonLogic struct for instance-based usage
   - Implement new instance methods that leverage arena allocation
   - Keep static methods functioning with traditional allocation

## Proposed API

```rust
// Updated JsonLogic structure
pub struct JsonLogic {
    // Existing fields
    custom_operators: RwLock<HashMap<String, CustomOperatorBox>>,
    // New fields
    arena: JsonLogicArena,
}

impl JsonLogic {
    // New instance constructor
    pub fn new() -> Self {
        Self {
            custom_operators: RwLock::new(HashMap::new()),
            arena: JsonLogicArena::new(),
        }
    }

    // Flexible prepare method that accepts different rule sources
    pub fn prepare<R: IntoRule>(&self, rule_source: R) -> Result<ArenaRule, Error> {
        rule_source.into_rule(self)
    }
    
    pub fn apply(&self, rule: &ArenaRule, data: &Value) -> JsonLogicResult {
        // Evaluate rule using arena allocation
    }
    
    // Existing static methods (unchanged for backward compatibility)
    pub fn apply_static(rule: &Rule, data: &Value) -> JsonLogicResult {
        // Existing implementation
    }
}

// Trait for converting different sources into rules
pub trait IntoRule {
    fn into_rule<'a>(&self, logic: &'a JsonLogic) -> Result<ArenaRule<'a>, Error>;
}

// Implement for Value
impl IntoRule for Value {
    fn into_rule<'a>(&self, logic: &'a JsonLogic) -> Result<ArenaRule<'a>, Error> {
        // Parse Value into ArenaRule
    }
}

// Implement for &str
impl IntoRule for &str {
    fn into_rule<'a>(&self, logic: &'a JsonLogic) -> Result<ArenaRule<'a>, Error> {
        // Parse string into ArenaRule
    }
}

// Backward compatibility layer
impl JsonLogic {
    // Static method that forwards to apply_static for backward compatibility
    pub fn apply(rule: &Rule, data: &Value) -> JsonLogicResult {
        Self::apply_static(rule, data)
    }
}
```

## Migration Examples

### Existing Code (continues to work)

```rust
use datalogic_rs::{JsonLogic, Rule};
use serde_json::json;

// Parse rule using traditional allocation
let rule = Rule::from_value(&json!({">": [{"var": "score"}, 50]})).unwrap();

// Evaluate using static method (unchanged behavior)
let data = json!({"score": 75});
let result = JsonLogic::apply(&rule, &data).unwrap();
assert_eq!(result, json!(true));
```

### New Code (using arena allocation)

```rust
use datalogic_rs::JsonLogic;
use serde_json::json;

// Create instance with arena
let logic = JsonLogic::new();

// Prepare rule from JSON Value
let rule_value = json!({">": [{"var": "score"}, 50]});
let rule = logic.prepare(&rule_value).unwrap();

// Evaluate using instance method with arena
let data = json!({"score": 75});
let result = logic.apply(&rule, &data).unwrap();
assert_eq!(result, json!(true));

// Or prepare rule directly from string
let rule_str = r#"{">": [{"var": "score"}, 50]}"#;
let rule = logic.prepare(rule_str).unwrap();
let result = logic.apply(&rule, &data).unwrap();
```

## Technical Implementation

1. **Unified Rule Creation**
   - Trait-based approach for flexible rule sources
   - Optimized parsing paths for different input types
   - Direct string parsing without intermediate Value allocation

2. **Dual Rule Types**
   - Traditional `Rule` type for backward compatibility
   - New `ArenaRule<'a>` type for arena-allocated rules
   - Conversion methods between the two types where needed

3. **Shared Evaluation Logic**
   - Core evaluation logic shared between both allocation strategies
   - Specialized optimizations for arena-allocated rules
   - Performance improvements available only to arena-based evaluation

## Benefits

1. **Flexible API**
   - Support for both Value and string-based rule creation
   - Unified interface for different input formats
   - Optimized parsing paths for each input type

2. **Backward Compatibility**
   - Existing code continues to work without changes
   - No breaking changes to public API
   - Gradual migration path for existing applications

3. **Performance Improvements**
   - Significant performance gains for new code using arena allocation
   - Direct string parsing without intermediate allocations
   - Reduced memory usage and fewer allocations

## Implementation Scope

- Update JsonLogic struct to include arena
- Implement IntoRule trait for different rule sources
- Create unified prepare method for rule creation
- Maintain backward compatibility with existing static methods
- Create dual rule types for different allocation strategies
- Provide comprehensive documentation for migration
- Add tests for both allocation strategies and input formats
- Benchmark performance differences

## Dependencies

- Requires the arena allocation system to be implemented first
- Will be part of the v3.0 API redesign

/// # DataValue
///
/// A memory-efficient value type that leverages arena allocation.
/// This replaces the direct dependency on `serde_json::Value` with a custom
/// implementation optimized for rule evaluation.
#[derive(Debug, Clone, PartialEq)]
pub enum DataValue<'a> {
    /// Represents a null value
    Null,
    
    /// Represents a boolean value
    Bool(bool),
    
    /// Represents a numeric value (integer or floating point)
    Number(NumberValue),
    
    /// Represents a string value (arena-allocated)
    String(&'a str),
    
    /// Represents an array of values (arena-allocated)
    Array(&'a [DataValue<'a>]),
    
    /// Represents an object with key-value pairs (arena-allocated)
    Object(&'a [(KeyRef<'a>, DataValue<'a>)]),
}

/// # NumberValue
///
/// Specialized representation for numeric values to optimize memory usage
/// based on the actual value.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum NumberValue {
    /// Integer value
    Integer(i64),
    
    /// Floating point value
    Float(f64),
}

/// # KeyRef
///
/// Represents an interned string key for object properties.
/// This reduces memory usage by deduplicating common keys.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct KeyRef<'a>(&'a str);

/// # DataArena
///
/// Memory arena for allocating DataValue instances and their components.
pub struct DataArena {
    // Internal implementation using bumpalo or similar
    arena: bumpalo::Bump,
    string_interner: StringInterner,
}

/// # StringInterner
///
/// Deduplicates strings to reduce memory usage.
pub struct StringInterner {
    strings: std::collections::HashMap<&'static str, ()>,
}

/// # ArenaRule
///
/// A rule that uses arena-allocated DataValue instances.
pub struct ArenaRule<'a> {
    // Rule implementation using DataValue
    root: &'a DataValue<'a>,
}

/// # DataLogic
///
/// The main entry point for evaluating logic rules with arena allocation.
pub struct DataLogic {
    // Existing fields
    custom_operators: std::sync::RwLock<std::collections::HashMap<String, CustomOperatorBox>>,
    // New fields
    arena: DataArena,
}

impl DataLogic {
    /// Creates a new DataLogic evaluator with an arena.
    pub fn new() -> Self {
        Self {
            custom_operators: std::sync::RwLock::new(std::collections::HashMap::new()),
            arena: DataArena::new(),
        }
    }
    
    /// Prepares a rule for evaluation using arena allocation.
    pub fn prepare<R: IntoRule>(&self, rule_source: R) -> Result<ArenaRule, Error> {
        rule_source.into_rule(self)
    }
    
    /// Evaluates a prepared rule against data.
    pub fn apply(&self, rule: &ArenaRule, data: &DataValue) -> Result<DataValue, Error> {
        // Implementation
        todo!()
    }
    
    /// Backward compatibility method.
    pub fn apply_static(rule: &Rule, data: &serde_json::Value) -> Result<serde_json::Value, Error> {
        // Existing implementation
        todo!()
    }
}

/// # IntoRule
///
/// Trait for converting different sources into rules.
pub trait IntoRule {
    fn into_rule<'a>(&self, logic: &'a DataLogic) -> Result<ArenaRule<'a>, Error>;
}

// Implement for serde_json::Value
impl IntoRule for serde_json::Value {
    fn into_rule<'a>(&self, logic: &'a DataLogic) -> Result<ArenaRule<'a>, Error> {
        // Parse Value into ArenaRule
        todo!()
    }
}

// Implement for &str
impl IntoRule for &str {
    fn into_rule<'a>(&self, logic: &'a DataLogic) -> Result<ArenaRule<'a>, Error> {
        // Parse string into ArenaRule
        todo!()
    }
}

// Conversion methods between DataValue and serde_json::Value
impl<'a> DataValue<'a> {
    /// Converts a DataValue to a serde_json::Value
    pub fn to_json_value(&self) -> serde_json::Value {
        match self {
            DataValue::Null => serde_json::Value::Null,
            DataValue::Bool(b) => serde_json::Value::Bool(*b),
            DataValue::Number(n) => match n {
                NumberValue::Integer(i) => serde_json::Value::Number((*i).into()),
                NumberValue::Float(f) => {
                    let n = serde_json::Number::from_f64(*f).unwrap_or_else(|| 0.0.into());
                    serde_json::Value::Number(n)
                }
            },
            DataValue::String(s) => serde_json::Value::String(s.to_string()),
            DataValue::Array(arr) => {
                let values: Vec<serde_json::Value> = arr.iter().map(|v| v.to_json_value()).collect();
                serde_json::Value::Array(values)
            },
            DataValue::Object(obj) => {
                let mut map = serde_json::Map::new();
                for (k, v) in obj.iter() {
                    map.insert(k.0.to_string(), v.to_json_value());
                }
                serde_json::Value::Object(map)
            }
        }
    }
    
    /// Creates a DataValue from a serde_json::Value using the provided arena
    pub fn from_json_value<'b>(value: &serde_json::Value, arena: &'b DataArena) -> DataValue<'b> {
        // Implementation
        todo!()
    }
}

// Implementation for DataArena
impl DataArena {
    /// Creates a new arena
    pub fn new() -> Self {
        Self {
            arena: bumpalo::Bump::new(),
            string_interner: StringInterner::new(),
        }
    }
    
    /// Allocates a string in the arena
    pub fn alloc_str(&self, s: &str) -> &str {
        self.arena.alloc_str(s)
    }
    
    /// Interns a string for reuse
    pub fn intern_str(&self, s: &str) -> &str {
        self.string_interner.intern(s, &self.arena)
    }
    
    /// Allocates an array of DataValues
    pub fn alloc_array<'a>(&'a self, values: &[DataValue<'a>]) -> &'a [DataValue<'a>] {
        self.arena.alloc_slice_copy(values)
    }
    
    /// Allocates an object (key-value pairs)
    pub fn alloc_object<'a>(&'a self, entries: &[(KeyRef<'a>, DataValue<'a>)]) -> &'a [(KeyRef<'a>, DataValue<'a>)] {
        self.arena.alloc_slice_copy(entries)
    }
    
    /// Creates a DataValue in the arena
    pub fn create_value<'a>(&'a self, value: DataValue<'a>) -> &'a DataValue<'a> {
        self.arena.alloc(value)
    }
    
    /// Resets the arena, freeing all allocations
    pub fn reset(&mut self) {
        self.arena.reset();
        self.string_interner = StringInterner::new();
    }
}

// Implementation for StringInterner
impl StringInterner {
    /// Creates a new string interner
    pub fn new() -> Self {
        Self {
            strings: std::collections::HashMap::new(),
        }
    }
    
    /// Interns a string, returning a reference to the unique instance
    pub fn intern<'a>(&self, s: &str, arena: &'a bumpalo::Bump) -> &'a str {
        // Implementation that uses the arena to store unique strings
        todo!()
    }
}
