use crate::opcode::OpCode;
use regex::Regex;
use serde_json::Value;
use std::sync::Arc;

/// A pre-parsed path segment for compiled variable access.
#[derive(Debug, Clone)]
pub enum PathSegment {
    /// Object field access by key
    Field(Box<str>),
    /// Array element access by index
    Index(usize),
    /// Try as object key first, then as array index (for segments that could be either).
    /// Pre-parses the index at compile time to avoid runtime parsing.
    FieldOrIndex(Box<str>, usize),
}

/// Hint for reduce context resolution, detected at compile time.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ReduceHint {
    /// Normal path access (no reduce context)
    None,
    /// Path is exactly "current" — return reduce_current directly
    Current,
    /// Path is exactly "accumulator" — return reduce_accumulator directly
    Accumulator,
    /// Path starts with "current." — segments[0] is "current", use segments[1..] from reduce_current
    CurrentPath,
    /// Path starts with "accumulator." — segments[0] is "accumulator", use segments[1..] from reduce_accumulator
    AccumulatorPath,
}

/// Hint for metadata access (index/key), detected at compile time.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MetadataHint {
    /// Normal data access
    None,
    /// Access frame index metadata
    Index,
    /// Access frame key metadata
    Key,
}

/// A compiled node representing a single operation or value in the logic tree.
///
/// Nodes are created during the compilation phase and evaluated during execution.
/// Each node type is optimized for its specific purpose:
///
/// - **Value**: Static JSON values that don't require evaluation
/// - **Array**: Collections of nodes evaluated sequentially
/// - **BuiltinOperator**: Fast OpCode-based dispatch for built-in operators
/// - **CustomOperator**: User-defined operators with dynamic dispatch
/// - **StructuredObject**: Template objects for structure preservation
#[derive(Debug, Clone)]
pub enum CompiledNode {
    /// A static JSON value that requires no evaluation.
    ///
    /// Used for literals like numbers, strings, booleans, and null.
    Value { value: Value },

    /// An array of compiled nodes.
    ///
    /// Each node is evaluated in sequence, and the results are collected into a JSON array.
    /// Uses `Box<[CompiledNode]>` for memory efficiency.
    Array { nodes: Box<[CompiledNode]> },

    /// A built-in operator optimized with OpCode dispatch.
    ///
    /// The OpCode enum enables direct dispatch without string lookups,
    /// significantly improving performance for the 50+ built-in operators.
    BuiltinOperator {
        opcode: OpCode,
        args: Box<[CompiledNode]>,
    },

    /// A custom operator registered via `DataLogic::add_operator`.
    ///
    /// Custom operators use dynamic dispatch and are looked up by name
    /// from the engine's operator registry.
    CustomOperator {
        name: String,
        args: Box<[CompiledNode]>,
    },

    /// A structured object template for preserve_structure mode.
    ///
    /// When structure preservation is enabled, objects with keys that are not
    /// built-in operators or registered custom operators are preserved as templates.
    /// Each field is evaluated independently, allowing for dynamic object generation.
    ///
    /// Note: Custom operators are checked before treating keys as structured fields,
    /// ensuring they work correctly within preserved structures.
    StructuredObject {
        fields: Box<[(String, CompiledNode)]>,
    },

    /// A pre-compiled variable access (unified var/val).
    ///
    /// scope_level 0 = current context (var-style), N = go up N levels (val with [[N], ...]).
    /// Segments are pre-parsed at compile time to avoid runtime string splitting.
    CompiledVar {
        scope_level: u32,
        segments: Box<[PathSegment]>,
        reduce_hint: ReduceHint,
        metadata_hint: MetadataHint,
        default_value: Option<Box<CompiledNode>>,
    },

    /// A pre-compiled exists check.
    ///
    /// scope_level 0 = current context, N = go up N levels.
    /// Segments are pre-parsed at compile time.
    CompiledExists {
        scope_level: u32,
        segments: Box<[PathSegment]>,
    },

    /// A pre-compiled split with regex pattern.
    ///
    /// When the split operator's delimiter is a static regex pattern with named
    /// capture groups, the regex is compiled once during the compilation phase
    /// instead of on every evaluation.
    CompiledSplitRegex {
        /// The text argument (only the first arg of split)
        args: Box<[CompiledNode]>,
        /// Pre-compiled regex pattern
        regex: Arc<Regex>,
        /// Pre-extracted capture group names
        capture_names: Box<[Box<str>]>,
    },

    /// A pre-compiled throw with a static error object.
    ///
    /// When `throw` is called with a literal string, the error object
    /// `{"type": "..."}` is pre-built at compile time.
    CompiledThrow { error_obj: Value },
}

/// Compiled logic that can be evaluated multiple times across different data.
///
/// `CompiledLogic` represents a pre-processed JSONLogic expression that has been
/// optimized for repeated evaluation. It's thread-safe and can be shared across
/// threads using `Arc`.
///
/// # Performance Benefits
///
/// - **Parse once, evaluate many**: Avoid repeated JSON parsing
/// - **Static evaluation**: Constant expressions are pre-computed
/// - **OpCode dispatch**: Built-in operators use fast enum dispatch
/// - **Thread-safe sharing**: Use `Arc` to share across threads
///
/// # Example
///
/// ```rust
/// use datalogic_rs::DataLogic;
/// use serde_json::json;
/// use std::sync::Arc;
///
/// let engine = DataLogic::new();
/// let logic = json!({">": [{"var": "score"}, 90]});
/// let compiled = engine.compile(&logic).unwrap(); // Returns Arc<CompiledLogic>
///
/// // Can be shared across threads
/// let compiled_clone = Arc::clone(&compiled);
/// std::thread::spawn(move || {
///     let data = json!({"score": 95});
///     let result = engine.evaluate_owned(&compiled_clone, data);
/// });
/// ```
#[derive(Debug, Clone)]
pub struct CompiledLogic {
    /// The root node of the compiled logic tree
    pub root: CompiledNode,
}

impl CompiledLogic {
    /// Creates a new compiled logic from a root node.
    ///
    /// # Arguments
    ///
    /// * `root` - The root node of the compiled logic tree
    pub fn new(root: CompiledNode) -> Self {
        Self { root }
    }

    /// Check if this compiled logic is static (can be evaluated without context)
    pub fn is_static(&self) -> bool {
        node_is_static(&self.root)
    }
}

/// Check if a compiled node is static (can be evaluated without runtime context).
pub(crate) fn node_is_static(node: &CompiledNode) -> bool {
    match node {
        CompiledNode::Value { .. } => true,
        CompiledNode::Array { nodes, .. } => nodes.iter().all(node_is_static),
        CompiledNode::BuiltinOperator { opcode, args, .. } => opcode_is_static(opcode, args),
        CompiledNode::CustomOperator { .. } => false,
        CompiledNode::CompiledVar { .. } | CompiledNode::CompiledExists { .. } => false,
        CompiledNode::CompiledSplitRegex { args, .. } => args.iter().all(node_is_static),
        CompiledNode::CompiledThrow { .. } => false,
        CompiledNode::StructuredObject { fields, .. } => {
            fields.iter().all(|(_, node)| node_is_static(node))
        }
    }
}

/// Check if an operator can be statically evaluated at compile time.
///
/// Static operators can be pre-computed during compilation when their arguments
/// are also static, eliminating runtime evaluation overhead.
///
/// # Classification Criteria
///
/// An operator is **non-static** (dynamic) if it:
/// 1. Reads from the data context (`var`, `val`, `missing`, `exists`)
/// 2. Uses iterative callbacks with changing context (`map`, `filter`, `reduce`)
/// 3. Has side effects or error handling (`try`, `throw`)
/// 4. Depends on runtime state (`now` for current time)
/// 5. Needs runtime disambiguation (`preserve`, `merge`, `min`, `max`)
///
/// All other operators are **static** when their arguments are static.
pub(crate) fn opcode_is_static(opcode: &OpCode, args: &[CompiledNode]) -> bool {
    use OpCode::*;

    // Check if all arguments are static first (common pattern)
    let args_static = || args.iter().all(node_is_static);

    match opcode {
        // Context-dependent: These operators read from the data context, which is
        // not available at compile time. They must remain dynamic.
        Var | Val | Missing | MissingSome | Exists => false,

        // Iteration operators: These push new contexts for each iteration and use
        // callbacks that may reference the iteration variable. Even with static
        // arrays, the callback logic depends on the per-element context.
        Map | Filter | Reduce | All | Some | None => false,

        // Error handling: These have control flow effects (early exit, error propagation)
        // that should be preserved for runtime execution.
        Try | Throw => false,

        // Time-dependent: Returns current UTC time, inherently non-static.
        Now => false,

        // Runtime disambiguation needed:
        // - Preserve: Must know it was explicitly used as an operator, not inferred
        // - Merge/Min/Max: Need to distinguish [1,2,3] literal from operator arguments
        //   at runtime to handle nested arrays correctly
        Preserve => false,
        Merge | Min | Max => false,

        // Pure operators: Static when all arguments are static. These perform
        // deterministic transformations without side effects or context access.
        Type | StartsWith | EndsWith | Upper | Lower | Trim | Split | Datetime | Timestamp
        | ParseDate | FormatDate | DateDiff | Abs | Ceil | Floor | Add | Subtract | Multiply
        | Divide | Modulo | Equals | StrictEquals | NotEquals | StrictNotEquals | GreaterThan
        | GreaterThanEqual | LessThan | LessThanEqual | Not | DoubleNot | And | Or | Ternary
        | If | Cat | Substr | In | Length | Sort | Slice | Coalesce | Switch => args_static(),
    }
}

/// Convert path segments back to a dot-separated path string.
pub(crate) fn segments_to_dot_path(segments: &[PathSegment]) -> String {
    segments
        .iter()
        .map(|seg| match seg {
            PathSegment::Field(s) | PathSegment::FieldOrIndex(s, _) => s.to_string(),
            PathSegment::Index(i) => i.to_string(),
        })
        .collect::<Vec<_>>()
        .join(".")
}

/// Convert a path segment to a JSON value.
pub(crate) fn segment_to_value(seg: &PathSegment) -> Value {
    match seg {
        PathSegment::Field(s) | PathSegment::FieldOrIndex(s, _) => Value::String(s.to_string()),
        PathSegment::Index(i) => Value::Number((*i as u64).into()),
    }
}

/// Convert a compiled node back to a JSON value (for custom operators).
pub(crate) fn node_to_value(node: &CompiledNode) -> Value {
    match node {
        CompiledNode::Value { value, .. } => value.clone(),
        CompiledNode::Array { nodes, .. } => {
            Value::Array(nodes.iter().map(node_to_value).collect())
        }
        CompiledNode::BuiltinOperator { opcode, args, .. } => {
            let mut obj = serde_json::Map::new();
            let args_value = if args.len() == 1 {
                node_to_value(&args[0])
            } else {
                Value::Array(args.iter().map(node_to_value).collect())
            };
            obj.insert(opcode.as_str().into(), args_value);
            Value::Object(obj)
        }
        CompiledNode::CustomOperator { name, args, .. } => {
            let mut obj = serde_json::Map::new();
            let args_value = if args.len() == 1 {
                node_to_value(&args[0])
            } else {
                Value::Array(args.iter().map(node_to_value).collect())
            };
            obj.insert(name.clone(), args_value);
            Value::Object(obj)
        }
        CompiledNode::StructuredObject { fields, .. } => {
            let mut obj = serde_json::Map::new();
            for (key, node) in fields {
                obj.insert(key.clone(), node_to_value(node));
            }
            Value::Object(obj)
        }
        CompiledNode::CompiledVar {
            scope_level,
            segments,
            default_value,
            ..
        } => {
            let mut obj = serde_json::Map::new();
            if *scope_level == 0 {
                // Reconstruct as var
                let path = segments_to_dot_path(segments);
                match default_value {
                    Some(def) => {
                        obj.insert(
                            "var".into(),
                            Value::Array(vec![Value::String(path), node_to_value(def)]),
                        );
                    }
                    None => {
                        obj.insert("var".into(), Value::String(path));
                    }
                }
            } else {
                // Reconstruct as val with level
                let mut arr: Vec<Value> = vec![Value::Array(vec![Value::Number(
                    (*scope_level as u64).into(),
                )])];
                for seg in segments.iter() {
                    arr.push(segment_to_value(seg));
                }
                obj.insert("val".into(), Value::Array(arr));
            }
            Value::Object(obj)
        }
        CompiledNode::CompiledExists { segments, .. } => {
            let mut obj = serde_json::Map::new();
            if segments.len() == 1 {
                obj.insert("exists".into(), segment_to_value(&segments[0]));
            } else {
                let arr: Vec<Value> = segments.iter().map(segment_to_value).collect();
                obj.insert("exists".into(), Value::Array(arr));
            }
            Value::Object(obj)
        }
        CompiledNode::CompiledSplitRegex { args, regex, .. } => {
            let mut obj = serde_json::Map::new();
            let mut arr = vec![node_to_value(&args[0])];
            arr.push(Value::String(regex.as_str().to_string()));
            obj.insert("split".into(), Value::Array(arr));
            Value::Object(obj)
        }
        CompiledNode::CompiledThrow { error_obj } => {
            let mut obj = serde_json::Map::new();
            // Extract the type string from the pre-built error object
            if let Value::Object(err_map) = error_obj {
                if let Some(Value::String(s)) = err_map.get("type") {
                    obj.insert("throw".into(), Value::String(s.clone()));
                } else {
                    obj.insert("throw".into(), error_obj.clone());
                }
            } else {
                obj.insert("throw".into(), error_obj.clone());
            }
            Value::Object(obj)
        }
    }
}
