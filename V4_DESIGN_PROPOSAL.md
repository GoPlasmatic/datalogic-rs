# DataLogic v4 Design Proposal

## Overview

Version 4 represents a fundamental redesign of the DataLogic library, focusing on simplicity, maintainability, and thread safety while removing complex lifetime management and arena allocation.

## Core Design Principles

### 1. Simplified Value System
- **Use `serde_json::Value` exclusively** for all data representation
- Remove custom `DataValue` enum and associated conversion overhead
- Leverage serde_json's mature ecosystem and optimizations
- Direct JSON manipulation without intermediate representations
- **Pass `&Value` references throughout** to minimize cloning
- Only clone when absolutely necessary (e.g., constructing new values)

### 2. No Arena Allocation
- **Remove all arena-based memory management**
- Eliminate `DataArena`, `BumpAllocator`, and complex lifetime parameters
- Use standard Rust ownership with references wherever possible
- Minimize cloning through careful API design with `&Value`
- Trust Rust's allocator and ownership system for memory efficiency

### 3. Clean Lifetime Management
- **Simple lifetime parameters only where needed** (for holding references)
- Remove complex lifetime annotations from core types
- Use borrowed references (`&Value`) for inputs and processing
- Only use owned values (`Value`) for final results
- Balance zero-copy performance with API ergonomics

### 4. Compiled Logic Architecture
- **Maintain compilation step for performance**
- Create a `CompiledLogic` type that pre-processes rules
- Cache operator lookups and structure analysis during compilation
- Reusable compiled logic instances across evaluations

### 5. Thread Safety
- **Make `CompiledLogic` shareable across threads**
- Implement `Send + Sync` for all core types
- Use `Arc<CompiledLogic>` for shared ownership
- Enable parallel rule evaluation without locks

### 6. Simplified Architecture
- **Remove over-engineered abstractions**
- Flatten module hierarchy where possible
- Direct operator implementations without complex traits
- Clear separation between compilation and evaluation phases

## Reference-Based Design Philosophy

### Core Principle: Minimize Cloning with Cow
- **Pervasive use of `Cow<'_, Value>`** - All value passing uses Cow for automatic clone avoidance
- **Input values as `Cow`** - Functions accept `Cow<'_, Value>` to handle both borrowed and owned
- **Operators return `Cow`** - Return borrowed data when possible, owned only when necessary
- **Smart cloning** - Cow automatically decides when to clone based on usage
- **Zero-copy by default** - Borrowing is preferred, cloning is deferred

### Memory Efficiency Strategies with Cow
1. **Cow-based data flow** - All values passed as `Cow<'_, Value>` throughout the pipeline
2. **Automatic lazy cloning** - Cow handles clone-on-write semantics transparently
3. **Reference counting for shared data** - Use `Arc` for compiled logic sharing
4. **Operator chaining without copies** - Pass `Cow::Borrowed` through pipeline until mutation needed
5. **Smart ownership transfer** - Use `Cow::into_owned()` only when ownership is required

### Example: Zero-Copy Operations with Cow
```rust
use std::borrow::Cow;

// All operations use Cow for automatic clone avoidance
impl CompiledLogic {
    fn evaluate_var<'a>(&self, path: &str, data: Cow<'a, Value>) -> Result<Cow<'a, Value>> {
        // Returns borrowed reference when possible
        match data.as_ref().pointer(path) {
            Some(val) => Ok(Cow::Borrowed(val)),
            None => Err("Variable not found")
        }
    }
    
    fn evaluate_comparison<'a>(
        &self, 
        left: Cow<'a, Value>, 
        right: Cow<'a, Value>
    ) -> Result<Cow<'a, Value>> {
        // Only creates new Value, inputs remain borrowed
        Ok(Cow::Owned(Value::Bool(left.as_ref() == right.as_ref())))
    }
    
    fn evaluate_transform<'a>(&self, input: Cow<'a, Value>) -> Result<Cow<'a, Value>> {
        // Smart cloning - only clone if we need to modify
        if needs_modification(&input) {
            let mut owned = input.into_owned();
            modify(&mut owned);
            Ok(Cow::Owned(owned))
        } else {
            Ok(input)  // Pass through without cloning
        }
    }
}
```

## Cow-Based Value Management

### Why Cow<'_, Value> Everywhere?

Using `Cow<'_, Value>` pervasively provides several key advantages:

1. **Automatic Clone Avoidance** - Cow decides at runtime whether to borrow or clone
2. **Flexible APIs** - Functions can accept both borrowed and owned values seamlessly
3. **Performance Transparency** - Clone only happens when mutation is required
4. **Composability** - Operations can be chained without forcing clones at each step
5. **Memory Efficiency** - Most read operations remain zero-copy

### Cow Patterns in Practice

```rust
// Pattern 1: Read-through operations
fn process_read_only<'a>(value: Cow<'a, Value>) -> Result<Cow<'a, Value>> {
    // If we're just reading, pass through the Cow unchanged
    if value.as_ref().is_object() {
        Ok(value)  // No clone!
    } else {
        Err("Not an object")
    }
}

// Pattern 2: Conditional modification
fn process_maybe_modify<'a>(value: Cow<'a, Value>) -> Result<Cow<'a, Value>> {
    if needs_modification(value.as_ref()) {
        // Only clone when we need to modify
        let mut owned = value.into_owned();
        modify(&mut owned);
        Ok(Cow::Owned(owned))
    } else {
        Ok(value)  // Pass through without cloning
    }
}

// Pattern 3: Creating new values
fn create_result<'a>(input: Cow<'a, Value>) -> Result<Cow<'a, Value>> {
    // New values are always Owned
    let result = compute_result(input.as_ref());
    Ok(Cow::Owned(result))
}

// Pattern 4: Extracting sub-values
fn extract_field<'a>(obj: Cow<'a, Value>, field: &str) -> Result<Cow<'a, Value>> {
    match obj.as_ref().get(field) {
        Some(val) => Ok(Cow::Borrowed(val)),  // Borrow from input
        None => Err("Field not found")
    }
}
```

### Performance Characteristics

| Operation | Without Cow | With Cow | Improvement |
|-----------|------------|----------|-------------|
| Read-only traversal | Clones at each step | Zero clones | 100% reduction |
| Conditional modification | Always clones | Clones only if modified | 50-90% reduction |
| Pipeline of 5 operations | 5 clones | 0-1 clones | 80-100% reduction |
| Extracting nested value | Clone entire path | Borrow directly | O(1) vs O(n) |

## Context Stack Architecture

### Core Concept

The context stack enables operators to access data at different scope levels during evaluation. This is critical for array operators like `map`, `filter`, and `reduce` that need to provide access to both the current item and parent scopes.

### Context Stack Design

```rust
use std::borrow::Cow;
use serde_json::Value;

/// Context stack for nested evaluations
pub struct ContextStack<'a> {
    /// Stack of context frames, with the root data at index 0
    frames: Vec<ContextFrame<'a>>,
}

/// A single frame in the context stack
pub struct ContextFrame<'a> {
    /// The data value at this context level
    data: Cow<'a, Value>,
    /// Optional metadata for this frame (e.g., "index" in map operations)
    metadata: Option<HashMap<String, Cow<'a, Value>>>,
}

impl<'a> ContextStack<'a> {
    /// Create a new context stack with root data
    pub fn new(root: Cow<'a, Value>) -> Self {
        Self {
            frames: vec![ContextFrame {
                data: root,
                metadata: None,
            }],
        }
    }
    
    /// Push a new context frame for nested evaluation
    pub fn push(&mut self, data: Cow<'a, Value>) {
        self.frames.push(ContextFrame {
            data,
            metadata: None,
        });
    }
    
    /// Push a frame with metadata (e.g., for map with index)
    pub fn push_with_metadata(
        &mut self, 
        data: Cow<'a, Value>,
        metadata: HashMap<String, Cow<'a, Value>>
    ) {
        self.frames.push(ContextFrame {
            data,
            metadata: Some(metadata),
        });
    }
    
    /// Pop the current context frame
    pub fn pop(&mut self) -> Option<ContextFrame<'a>> {
        // Never pop the root frame
        if self.frames.len() > 1 {
            self.frames.pop()
        } else {
            None
        }
    }
    
    /// Access data at a context level relative to current
    /// The sign is ignored - both positive and negative mean the same thing
    /// - 0: current context
    /// - 1 or -1: go up 1 level (parent)
    /// - 2 or -2: go up 2 levels (grandparent)
    /// - N or -N: go up N levels
    pub fn get_at_level(&self, level: isize) -> Option<&ContextFrame<'a>> {
        // Get absolute value - sign doesn't matter
        let levels_up = level.abs() as usize;
        
        if levels_up == 0 {
            // 0 means current context
            return self.frames.last();
        }
        
        let current_index = self.frames.len() - 1;
        
        if levels_up > current_index {
            // Going up more levels than exist, return root
            return self.frames.first();
        }
        
        // Calculate target index by going up from current
        let target_index = current_index - levels_up;
        self.frames.get(target_index)
    }
    
    /// Get the current context frame (top of stack)
    pub fn current(&self) -> &ContextFrame<'a> {
        self.frames.last().expect("Context stack should never be empty")
    }
    
    /// Get the root context frame
    pub fn root(&self) -> &ContextFrame<'a> {
        &self.frames[0]
    }
}
```

### Variable Access with Context Levels

The `val` operator (and `var` for JSONLogic compatibility) supports accessing different context levels:

```rust
impl ValOperator {
    fn evaluate<'a>(
        &self,
        args: &[Cow<'a, Value>],
        context: &ContextStack<'a>
    ) -> Result<Cow<'a, Value>> {
        if args.is_empty() {
            // No args means current context
            return Ok(context.current().data.clone());
        }
        
        let path_value = &args[0];
        
        // Handle array notation for context levels: [[level], "path"]
        // Level indicates how many levels to go up from current
        // Sign doesn't matter: [1] and [-1] both mean parent
        // [2] and [-2] both mean grandparent, etc.
        if let Some(arr) = path_value.as_array() {
            if arr.len() == 2 {
                if let Some(level_arr) = arr[0].as_array() {
                    if let Some(level) = level_arr.first()
                        .and_then(|v| v.as_i64()) {
                        
                        // Get frame at relative level
                        // Both [1] and [-1] go up 1 level to parent
                        // Both [2] and [-2] go up 2 levels to grandparent
                        let frame = context.get_at_level(level as isize)
                            .ok_or("Invalid context level")?;
                        
                        // Access path in that frame
                        let path = arr[1].as_str().ok_or("Invalid path")?;
                        
                        // Special handling for metadata keys like "index"
                        if let Some(metadata) = &frame.metadata {
                            if let Some(value) = metadata.get(path) {
                                return Ok(value.clone());
                            }
                        }
                        
                        // Normal path access in the target frame
                        return access_path(&frame.data, path);
                    }
                }
            }
        }
        
        // Standard path access in current context
        let path = path_value.as_str().unwrap_or("");
        access_path(&context.current().data, path)
    }
}
```

### Array Operators with Context Stack

Array operators manage the context stack by pushing/popping frames:

```rust
impl MapOperator {
    fn evaluate<'a>(
        &self,
        args: &[Cow<'a, Value>],
        context: &mut ContextStack<'a>,
        evaluator: &impl Evaluator
    ) -> Result<Cow<'a, Value>> {
        let collection = evaluator.evaluate(&args[0], context)?;
        let logic = &args[1];
        
        if let Some(arr) = collection.as_ref().as_array() {
            // Array iteration
            let mut results = Vec::with_capacity(arr.len());
            
            for (index, item) in arr.iter().enumerate() {
                // Create metadata with index
                let mut metadata = HashMap::new();
                metadata.insert("index".to_string(), 
                    Cow::Owned(Value::Number(index.into())));
                
                // Push new context with current item and metadata
                context.push_with_metadata(Cow::Borrowed(item), metadata);
                
                // Evaluate the logic in new context
                let result = evaluator.evaluate(logic, context)?;
                results.push(result.into_owned());
                
                // Pop the context frame
                context.pop();
            }
            
            Ok(Cow::Owned(Value::Array(results)))
        } else if let Some(obj) = collection.as_ref().as_object() {
            // Object iteration
            let mut results = Vec::with_capacity(obj.len());
            
            for (key, value) in obj.iter() {
                // Create metadata with key and index
                let mut metadata = HashMap::new();
                metadata.insert("key".to_string(), 
                    Cow::Owned(Value::String(key.clone())));
                metadata.insert("index".to_string(), 
                    Cow::Owned(Value::Number(results.len().into())));
                
                // Push new context with current value and metadata
                context.push_with_metadata(Cow::Borrowed(value), metadata);
                
                // Evaluate the logic in new context
                let result = evaluator.evaluate(logic, context)?;
                results.push(result.into_owned());
                
                // Pop the context frame
                context.pop();
            }
            
            Ok(Cow::Owned(Value::Array(results)))
        } else {
            Ok(collection)
        }
    }
}

impl FilterOperator {
    fn evaluate<'a>(
        &self,
        args: &[Cow<'a, Value>],
        context: &mut ContextStack<'a>,
        evaluator: &impl Evaluator
    ) -> Result<Cow<'a, Value>> {
        let collection = evaluator.evaluate(&args[0], context)?;
        let predicate = &args[1];
        
        if let Some(arr) = collection.as_ref().as_array() {
            // Array iteration
            let mut results = Vec::new();
            
            for (index, item) in arr.iter().enumerate() {
                // Create metadata with index
                let mut metadata = HashMap::new();
                metadata.insert("index".to_string(), 
                    Cow::Owned(Value::Number(index.into())));
                
                // Push new context with current item and metadata
                context.push_with_metadata(Cow::Borrowed(item), metadata);
                
                // Evaluate predicate
                let keep = evaluator.evaluate(predicate, context)?;
                
                // Pop context
                context.pop();
                
                if is_truthy(keep.as_ref()) {
                    results.push(item.clone());
                }
            }
            
            Ok(Cow::Owned(Value::Array(results)))
        } else if let Some(obj) = collection.as_ref().as_object() {
            // Object iteration - filter returns an object with kept key-value pairs
            let mut result_obj = serde_json::Map::new();
            let mut index = 0;
            
            for (key, value) in obj.iter() {
                // Create metadata with key and index
                let mut metadata = HashMap::new();
                metadata.insert("key".to_string(), 
                    Cow::Owned(Value::String(key.clone())));
                metadata.insert("index".to_string(), 
                    Cow::Owned(Value::Number(index.into())));
                
                // Push new context with current value and metadata
                context.push_with_metadata(Cow::Borrowed(value), metadata);
                
                // Evaluate predicate
                let keep = evaluator.evaluate(predicate, context)?;
                
                // Pop context
                context.pop();
                
                if is_truthy(keep.as_ref()) {
                    result_obj.insert(key.clone(), value.clone());
                }
                
                index += 1;
            }
            
            Ok(Cow::Owned(Value::Object(result_obj)))
        } else {
            // Non-iterable values return empty array/object
            Ok(Cow::Owned(Value::Array(vec![])))
        }
    }
}

impl ReduceOperator {
    fn evaluate<'a>(
        &self,
        args: &[Cow<'a, Value>],
        context: &mut ContextStack<'a>,
        evaluator: &impl Evaluator
    ) -> Result<Cow<'a, Value>> {
        let array = evaluator.evaluate(&args[0], context)?;
        let logic = &args[1];
        let initial = evaluator.evaluate(&args[2], context)?;
        
        if let Some(arr) = array.as_ref().as_array() {
            let mut accumulator = initial.into_owned();
            
            for current in arr {
                // Create context with special "current" and "accumulator" vars
                let mut frame_data = Value::Object(serde_json::Map::new());
                frame_data["current"] = current.clone();
                frame_data["accumulator"] = accumulator.clone();
                
                context.push(Cow::Owned(frame_data));
                
                // Evaluate the reduction logic
                accumulator = evaluator.evaluate(logic, context)?.into_owned();
                
                context.pop();
            }
            
            Ok(Cow::Owned(accumulator))
        } else {
            Ok(initial)
        }
    }
}
```

### Context Stack Usage Examples

Based on the test cases:

```javascript
// Example 1: Access parent context in map
// Rule: {"map": [[1,2,3], {"+": [{"val": []}, {"val": [[-2], "adder"]}]}]}
// Data: {"adder": 10}
// Result: [11, 12, 13]
// 
// Context stack during evaluation:
// Frame 0: {"adder": 10} (root)
// Frame 1: 1 (current item) with metadata {"index": 0}
// {"val": []} returns 1 (current context, 0 levels up)
// {"val": [[-2], "adder"]} goes up 2 levels to root (sign ignored, |2| levels up)

// Example 2: Access index in array iteration
// Rule: {"map": [[1,2,3], {"+": [{"val": []}, {"val": [[1], "index"]}]}]}
// Result: [1, 3, 5]
//
// Frame 0: root data
// Frame 1: current item with metadata {"index": 0/1/2}
// {"val": [[1], "index"]} goes up 1 level and gets "index" metadata
// Note: [[1], "index"] and [[-1], "index"] would be equivalent

// Example 3: Object iteration with key access
// Rule: {"map": [{"a": 10, "b": 20}, {"cat": [{"val": [[1], "key"]}, ": ", {"val": []}]}]}
// Result: ["a: 10", "b: 20"]
//
// Frame 0: root data
// Frame 1: current value (10 or 20) with metadata {"key": "a"/"b", "index": 0/1}
// {"val": [[1], "key"]} accesses the key metadata

// Example 4: Filter using parent context
// Rule: {"filter": [{"val": "people"}, {"===": [{"val": "department"}, {"val": [[2], "department"]}]}]}
// Data: {"department": "Engineering", "people": [...]}
//
// Frame 0: {"department": "Engineering", "people": [...]} (root)
// Frame 1: current person object with metadata {"index": 0/1/2...}
// {"val": "department"} gets department from current person
// {"val": [[2], "department"]} goes up 2 levels to root to get parent department
```

### Metadata Available in Context Frames

Array and object iteration operators (`map`, `filter`, `all`, `some`, `none`) automatically add metadata to context frames:

**For Array Iteration:**
- `index`: The current index in the array (0-based)

**For Object Iteration:**
- `key`: The current property key
- `index`: The iteration order index (0-based)

This metadata can be accessed using the context level syntax:
- `{"val": [[1], "index"]}` - Get index from parent frame
- `{"val": [[1], "key"]}` - Get key from parent frame (object iteration only)

## Proposed API Design

```rust
use serde_json::Value;
use std::sync::Arc;

// Main engine - thread-safe and simple
pub struct DataLogic {
    custom_operators: HashMap<String, Box<dyn Operator>>,
}

// Compiled logic - immutable and shareable
pub struct CompiledLogic {
    root: CompiledNode,
    // Pre-computed metadata for optimization
}

// Operator trait using context stack for scope management
pub trait Operator: Send + Sync {
    fn evaluate<'a>(
        &self, 
        args: &[Cow<'a, Value>], 
        context: &mut ContextStack<'a>,
        evaluator: &dyn Evaluator<'a>
    ) -> Result<Cow<'a, Value>>;
}

// Evaluator trait for recursive evaluation
pub trait Evaluator<'a> {
    fn evaluate(
        &self,
        logic: &Cow<'a, Value>,
        context: &mut ContextStack<'a>
    ) -> Result<Cow<'a, Value>>;
}

// Example operator implementations with context stack
impl Operator for VarOperator {
    fn evaluate<'a>(
        &self, 
        args: &[Cow<'a, Value>], 
        context: &mut ContextStack<'a>,
        _evaluator: &dyn Evaluator<'a>
    ) -> Result<Cow<'a, Value>> {
        // Handle context-level access as shown in ValOperator above
        // This is a simplified version for standard var access
        let path = args[0].as_ref().as_str().unwrap_or("");
        match context.current().data.as_ref().pointer(path) {
            Some(val) => Ok(Cow::Borrowed(val)),
            None => Err("Variable not found".into())
        }
    }
}

impl Operator for EqualsOperator {
    fn evaluate<'a>(
        &self, 
        args: &[Cow<'a, Value>], 
        context: &mut ContextStack<'a>,
        evaluator: &dyn Evaluator<'a>
    ) -> Result<Cow<'a, Value>> {
        // Evaluate both arguments in current context
        let left = evaluator.evaluate(&args[0], context)?;
        let right = evaluator.evaluate(&args[1], context)?;
        
        // Compare without cloning, return new bool as Owned
        let result = left.as_ref() == right.as_ref();
        Ok(Cow::Owned(Value::Bool(result)))
    }
}

impl Operator for IfOperator {
    fn evaluate<'a>(
        &self,
        args: &[Cow<'a, Value>],
        context: &mut ContextStack<'a>,
        evaluator: &dyn Evaluator<'a>
    ) -> Result<Cow<'a, Value>> {
        // Support variadic if/elseif/else chains
        let mut i = 0;
        while i < args.len() {
            if i == args.len() - 1 {
                // Final else clause
                return evaluator.evaluate(&args[i], context);
            }
            
            // Evaluate condition
            let condition = evaluator.evaluate(&args[i], context)?;
            if is_truthy(condition.as_ref()) {
                // Evaluate then branch
                return evaluator.evaluate(&args[i + 1], context);
            }
            
            // Move to next if/elseif pair
            i += 2;
        }
        
        Ok(Cow::Owned(Value::Null))
    }
}

impl DataLogic {
    pub fn new() -> Self;
    
    // Compile once, use many times
    pub fn compile<'a>(&self, logic: Cow<'a, Value>) -> Result<Arc<CompiledLogic>>;
    
    // Primary evaluation method using context stack
    pub fn evaluate<'a>(
        &self,
        compiled: &CompiledLogic,
        data: Cow<'a, Value>,
    ) -> Result<Cow<'a, Value>> {
        // Initialize context stack with root data
        let mut context = ContextStack::new(data);
        
        // Evaluate the compiled logic with the context
        self.evaluate_node(&compiled.root, &mut context)
    }
    
    // Convenience for owned Values
    pub fn evaluate_owned(
        &self,
        compiled: &CompiledLogic,
        data: Value,
    ) -> Result<Value> {
        self.evaluate(compiled, Cow::Owned(data))
            .map(|cow| cow.into_owned())
    }
    
    // Convenience for borrowed Values
    pub fn evaluate_ref<'a>(
        &self,
        compiled: &CompiledLogic,
        data: &'a Value,
    ) -> Result<Cow<'a, Value>> {
        self.evaluate(compiled, Cow::Borrowed(data))
    }
    
    // Convenience method for JSON strings
    pub fn evaluate_json(
        &self,
        logic: &str,
        data: &str,
    ) -> Result<Value>;
}

// Usage example with Cow:
let engine = DataLogic::new();
let compiled = engine.compile(Cow::Borrowed(&logic_json))?;

// Share compiled logic across threads
let compiled_arc = Arc::clone(&compiled);
thread::spawn(move || {
    // Use Cow for automatic clone avoidance
    let result = engine.evaluate(&compiled_arc, Cow::Borrowed(&data))?;
    
    // Result is Cow - only convert to owned if needed
    if needs_owned_result {
        let owned = result.into_owned();
    } else {
        // Use borrowed data directly
        process_borrowed(result.as_ref());
    }
});

// Chain operations without cloning
let data = Cow::Borrowed(&input);
let step1 = engine.evaluate(&compiled1, data)?;
let step2 = engine.evaluate(&compiled2, step1)?;  // Passes Cow through
let final_result = step3.into_owned();  // Only clone at the end
```

## Migration Benefits

### Performance
- Compilation phase optimizes rule structure once
- No arena allocation overhead
- **Minimal cloning through pervasive use of `&Value`**
- **Zero-copy operations where possible**
- Better cache locality with standard allocations
- Parallel evaluation support

### Maintainability
- Simpler codebase without lifetime complexity
- Standard Rust patterns throughout
- Easier to understand and contribute to
- Reduced cognitive load for users

### Compatibility
- Direct `serde_json::Value` input/output
- No conversion layer needed
- **Reference-based APIs reduce memory overhead**
- Better integration with existing JSON ecosystems
- Simplified FFI and WASM bindings

## Implementation Phases

### Phase 1: Core Refactor
1. Replace DataValue with serde_json::Value
2. Remove arena allocation system
3. Implement new CompiledLogic structure
4. Update operator implementations to use `&Value` parameters
5. Design APIs to minimize Value cloning

### Phase 2: Thread Safety
1. Add Send + Sync bounds
2. Use Arc for compiled logic sharing
3. Ensure operator thread safety
4. Add parallel evaluation tests

### Phase 3: API Refinement
1. Simplify public API surface
2. Remove unnecessary abstractions
3. Improve error messages
4. Update documentation

### Phase 4: Optimization
1. Profile and optimize hot paths
2. Consider small-value optimizations
3. Implement operator result caching
4. Benchmark against v3

## Breaking Changes

### API Changes
- Remove all lifetime parameters from public types
- Change from `DataValue` to `serde_json::Value`
- New compilation step required before evaluation
- Custom operator trait signature changes

### Removed Features
- Arena allocation control
- Structure preservation mode (can be reimplemented simply)
- Complex custom operator APIs
- Direct string evaluation without compilation

## Risks and Mitigations

### Memory Usage
- **Risk**: More allocations without arena
- **Mitigation**: Use references (`&Value`) throughout; only clone when creating new values; profile real-world usage

### Performance
- **Risk**: Potential slowdown if cloning is required
- **Mitigation**: Reference-based APIs; lazy cloning with Cow when possible; compilation phase optimization; benchmark-driven development

### Backwards Compatibility
- **Risk**: Breaking changes for existing users
- **Mitigation**: Clear migration guide; maintain v3 branch for transition period

## Success Metrics

1. **Clone Reduction**: 80-95% reduction in unnecessary clones through Cow usage
2. **Memory Efficiency**: Measurable reduction in heap allocations for typical workloads
3. **Thread Safety**: Full Send + Sync support for core types
4. **Performance**: 20-40% performance improvement in benchmarks due to reduced cloning
5. **API Simplicity**: Unified Cow-based API that works with both borrowed and owned values
6. **Code Reduction**: Target 30-40% reduction in lines of code
7. **Zero-Copy Operations**: Majority of read operations achieve true zero-copy

## Timeline

- **Week 1-2**: Core refactor to serde_json::Value
- **Week 3**: Implement CompiledLogic and compilation phase
- **Week 4**: Add thread safety and parallel evaluation
- **Week 5**: Optimization and benchmarking
- **Week 6**: Documentation and migration guide

## Conclusion

Version 4 prioritizes simplicity and usability while maintaining performance through smart compilation and reference-based processing. By removing arena allocation but embracing reference-based APIs with `&Value` throughout, we achieve:

1. **Minimal memory overhead** - References avoid unnecessary cloning
2. **Simple lifetime management** - Only basic lifetimes for holding references
3. **Thread-safe design** - Immutable references enable safe concurrent access
4. **Clear performance model** - Clone only when creating new values
5. **Rust-idiomatic patterns** - Leverage borrowing and ownership effectively

This approach creates a more maintainable and performant library that better serves the Rust ecosystem's needs while avoiding the complexity of custom memory management.