use crate::opcode::OpCode;
#[cfg(feature = "ext-string")]
use regex::Regex;
use serde_json::Value;
#[cfg(feature = "ext-string")]
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

/// Sentinel id used for synthetic nodes built outside the compile pipeline
/// (test helpers, run-time value wrappers in `eager_apply`, etc.). Real IDs
/// are always nonzero since `CompileCtx` starts the counter at 1.
pub const SYNTHETIC_ID: u32 = 0;

/// Data for a custom operator (boxed inside CompiledNode to reduce enum size).
#[derive(Debug, Clone)]
pub struct CustomOperatorData {
    pub id: u32,
    pub name: String,
    pub args: Box<[CompiledNode]>,
}

/// Data for a structured object template (boxed inside CompiledNode to reduce enum size).
#[cfg(feature = "preserve")]
#[derive(Debug, Clone)]
pub struct StructuredObjectData {
    pub id: u32,
    pub fields: Box<[(String, CompiledNode)]>,
}

/// Data for a pre-compiled exists check (boxed inside CompiledNode to reduce enum size).
#[cfg(feature = "ext-control")]
#[derive(Debug, Clone)]
pub struct CompiledExistsData {
    pub id: u32,
    pub scope_level: u32,
    pub segments: Box<[PathSegment]>,
}

/// Data for a pre-compiled split with regex (boxed inside CompiledNode to reduce enum size).
#[cfg(feature = "ext-string")]
#[derive(Debug, Clone)]
pub struct CompiledSplitRegexData {
    pub id: u32,
    pub args: Box<[CompiledNode]>,
    pub regex: Arc<Regex>,
    pub capture_names: Box<[Box<str>]>,
}

/// Data for a pre-compiled throw with a static error object.
/// Previously `Box<Value>`; upgraded to a named struct so it can carry an id
/// alongside the error payload.
#[cfg(feature = "error-handling")]
#[derive(Debug, Clone)]
pub struct CompiledThrowData {
    pub id: u32,
    pub error: Value,
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
    Value { id: u32, value: Value },

    /// An array of compiled nodes.
    ///
    /// Each node is evaluated in sequence, and the results are collected into a JSON array.
    /// Uses `Box<[CompiledNode]>` for memory efficiency.
    Array { id: u32, nodes: Box<[CompiledNode]> },

    /// A built-in operator optimized with OpCode dispatch.
    ///
    /// The OpCode enum enables direct dispatch without string lookups,
    /// significantly improving performance for the 50+ built-in operators.
    BuiltinOperator {
        id: u32,
        opcode: OpCode,
        args: Box<[CompiledNode]>,
    },

    /// A custom operator registered via `DataLogic::add_operator`.
    /// Boxed to reduce enum size (rare variant).
    CustomOperator(Box<CustomOperatorData>),

    /// A structured object template for preserve_structure mode.
    /// Boxed to reduce enum size (rare variant).
    #[cfg(feature = "preserve")]
    StructuredObject(Box<StructuredObjectData>),

    /// A pre-compiled variable access (unified var/val).
    ///
    /// scope_level 0 = current context (var-style), N = go up N levels (val with [[N], ...]).
    /// Segments are pre-parsed at compile time to avoid runtime string splitting.
    CompiledVar {
        id: u32,
        scope_level: u32,
        segments: Box<[PathSegment]>,
        reduce_hint: ReduceHint,
        metadata_hint: MetadataHint,
        default_value: Option<Box<CompiledNode>>,
    },

    /// A pre-compiled exists check.
    /// Boxed to reduce enum size (rare variant).
    #[cfg(feature = "ext-control")]
    CompiledExists(Box<CompiledExistsData>),

    /// A pre-compiled split with regex pattern.
    /// Boxed to reduce enum size (rare variant).
    #[cfg(feature = "ext-string")]
    CompiledSplitRegex(Box<CompiledSplitRegexData>),

    /// A pre-compiled throw with a static error object.
    /// Boxed to reduce enum size (rare variant).
    #[cfg(feature = "error-handling")]
    CompiledThrow(Box<CompiledThrowData>),
}

impl CompiledNode {
    /// Returns the unique id assigned to this node during compilation.
    ///
    /// IDs are shared across tracing and error breadcrumbs — one source of
    /// truth per node. Synthetic nodes built outside the compile pipeline
    /// (test helpers, `eager_apply` value wrappers) carry [`SYNTHETIC_ID`].
    #[inline]
    pub fn id(&self) -> u32 {
        match self {
            CompiledNode::Value { id, .. } => *id,
            CompiledNode::Array { id, .. } => *id,
            CompiledNode::BuiltinOperator { id, .. } => *id,
            CompiledNode::CustomOperator(data) => data.id,
            #[cfg(feature = "preserve")]
            CompiledNode::StructuredObject(data) => data.id,
            CompiledNode::CompiledVar { id, .. } => *id,
            #[cfg(feature = "ext-control")]
            CompiledNode::CompiledExists(data) => data.id,
            #[cfg(feature = "ext-string")]
            CompiledNode::CompiledSplitRegex(data) => data.id,
            #[cfg(feature = "error-handling")]
            CompiledNode::CompiledThrow(data) => data.id,
        }
    }

    /// Convenience constructor for a `Value` node with a [`SYNTHETIC_ID`].
    ///
    /// Used by operator fast paths that wrap runtime values back into
    /// `CompiledNode::Value` purely for dispatch. These wrappers are never
    /// observed by tracing or error reporting, so assigning a real id would
    /// be misleading.
    #[inline]
    pub fn synthetic_value(value: Value) -> Self {
        CompiledNode::Value {
            id: SYNTHETIC_ID,
            value,
        }
    }

    /// Returns the name of this node's top-level operator, if any.
    ///
    /// Used when wrapping an error with structured context — we only report
    /// the outermost operator, not the full nested call chain.
    pub fn operator_name(&self) -> Option<String> {
        match self {
            CompiledNode::BuiltinOperator { opcode, .. } => Some(opcode.as_str().to_string()),
            CompiledNode::CustomOperator(data) => Some(data.name.clone()),
            CompiledNode::CompiledVar { .. } => Some("var".to_string()),
            #[cfg(feature = "ext-control")]
            CompiledNode::CompiledExists(_) => Some("exists".to_string()),
            #[cfg(feature = "ext-string")]
            CompiledNode::CompiledSplitRegex(_) => Some("split".to_string()),
            #[cfg(feature = "error-handling")]
            CompiledNode::CompiledThrow(_) => Some("throw".to_string()),
            _ => None,
        }
    }
}

/// Compile-time context for assigning unique node ids.
///
/// Threaded through `compile_node` so every node constructed during
/// compilation gets a fresh, monotonically increasing id. The counter starts
/// at 1 — id 0 is reserved for synthetic nodes (see [`SYNTHETIC_ID`]).
#[derive(Debug)]
pub(crate) struct CompileCtx {
    next_id: u32,
}

impl CompileCtx {
    pub(crate) fn new() -> Self {
        Self { next_id: 1 }
    }

    /// Allocate a fresh node id.
    #[inline]
    pub(crate) fn next_id(&mut self) -> u32 {
        let id = self.next_id;
        self.next_id = self.next_id.saturating_add(1);
        id
    }
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
    /// Conservative upper bound on the static portion of arena allocations
    /// this rule will need (literals, structured-object skeletons, etc.).
    /// Used to size the per-call `Bump` so the first chunk is large enough.
    /// `pub(crate)` — internal arena infrastructure.
    pub(crate) arena_static_bytes: usize,
    /// Pre-computed decision: should `evaluate()` route through the arena
    /// dispatch path? Cached at compile time so the per-call check is a
    /// single bool load instead of a tree pattern-match.
    pub(crate) uses_arena_dispatch: bool,
}

impl CompiledLogic {
    /// Creates a new compiled logic from a root node.
    ///
    /// # Arguments
    ///
    /// * `root` - The root node of the compiled logic tree
    pub fn new(root: CompiledNode) -> Self {
        let arena_static_bytes = estimate_arena_static_bytes(&root);
        let uses_arena_dispatch = root_uses_arena_pure(&root);
        Self {
            root,
            arena_static_bytes,
            uses_arena_dispatch,
        }
    }

    /// Check if this compiled logic is static (can be evaluated without context)
    pub fn is_static(&self) -> bool {
        node_is_static(&self.root)
    }
}

/// Pure compile-time predicate matching `engine::root_uses_arena`. Lives here
/// so the result can be cached on `CompiledLogic`. Keep in sync with the
/// dispatch logic in `engine.rs`.
fn root_uses_arena_pure(node: &CompiledNode) -> bool {
    match node {
        CompiledNode::BuiltinOperator {
            opcode: OpCode::Filter,
            args,
            ..
        } => arena_friendly_iter_input_pure(args),
        CompiledNode::BuiltinOperator {
            opcode: OpCode::Map,
            args,
            ..
        } => arena_friendly_iter_input_pure(args) && map_body_is_var_pure(args),
        CompiledNode::BuiltinOperator {
            opcode: OpCode::All | OpCode::Some | OpCode::None,
            args,
            ..
        } => arena_friendly_iter_input_pure(args),
        CompiledNode::BuiltinOperator {
            opcode: OpCode::Length,
            args,
            ..
        } => {
            args.len() == 1
                && matches!(
                    &args[0],
                    CompiledNode::BuiltinOperator { opcode, .. }
                        if matches!(
                            opcode,
                            OpCode::Filter | OpCode::Map | OpCode::All | OpCode::Some | OpCode::None
                        )
                )
        }
        _ => false,
    }
}

fn arena_friendly_iter_input_pure(args: &[CompiledNode]) -> bool {
    if args.is_empty() {
        return false;
    }
    matches!(
        &args[0],
        CompiledNode::CompiledVar {
            scope_level: 0,
            reduce_hint: ReduceHint::None,
            metadata_hint: MetadataHint::None,
            default_value: None,
            ..
        }
    ) || matches!(
        &args[0],
        CompiledNode::BuiltinOperator { opcode, .. }
            if matches!(
                opcode,
                OpCode::Filter | OpCode::Map | OpCode::All | OpCode::Some | OpCode::None
            )
    )
}

fn map_body_is_var_pure(args: &[CompiledNode]) -> bool {
    args.len() == 2
        && matches!(
            &args[1],
            CompiledNode::CompiledVar {
                scope_level: 0,
                reduce_hint: ReduceHint::None,
                metadata_hint: MetadataHint::None,
                default_value: None,
                ..
            }
        )
}

/// Estimate the static (rule-dependent, data-independent) portion of arena
/// bytes this rule will need at evaluation time. Conservative — overestimating
/// is harmless (one larger bumpalo chunk), underestimating costs an extra
/// chunk allocation. Data-dependent allocations (filter results, map outputs)
/// can't be predicted here.
fn estimate_arena_static_bytes(node: &CompiledNode) -> usize {
    // Base cost per node when promoted to ArenaValue: ~32 bytes for the enum +
    // a small fudge for slice headers. Add string content separately.
    const PER_NODE: usize = 48;
    let mut bytes = PER_NODE;
    match node {
        CompiledNode::Value { value, .. } => {
            bytes += estimate_value_bytes(value);
        }
        CompiledNode::Array { nodes, .. } => {
            for n in nodes.iter() {
                bytes += estimate_arena_static_bytes(n);
            }
        }
        CompiledNode::BuiltinOperator { args, .. } => {
            for n in args.iter() {
                bytes += estimate_arena_static_bytes(n);
            }
        }
        CompiledNode::CustomOperator(data) => {
            for n in data.args.iter() {
                bytes += estimate_arena_static_bytes(n);
            }
        }
        CompiledNode::CompiledVar { default_value, .. } => {
            if let Some(d) = default_value {
                bytes += estimate_arena_static_bytes(d);
            }
        }
        #[cfg(feature = "ext-control")]
        CompiledNode::CompiledExists(_) => {}
        #[cfg(feature = "ext-string")]
        CompiledNode::CompiledSplitRegex(data) => {
            for n in data.args.iter() {
                bytes += estimate_arena_static_bytes(n);
            }
        }
        #[cfg(feature = "error-handling")]
        CompiledNode::CompiledThrow(data) => {
            bytes += estimate_value_bytes(&data.error);
        }
        #[cfg(feature = "preserve")]
        CompiledNode::StructuredObject(data) => {
            for (k, n) in data.fields.iter() {
                bytes += k.len() + estimate_arena_static_bytes(n);
            }
        }
    }
    bytes
}

fn estimate_value_bytes(v: &Value) -> usize {
    match v {
        Value::String(s) => s.len() + 16,
        Value::Array(arr) => 16 + arr.iter().map(estimate_value_bytes).sum::<usize>(),
        Value::Object(obj) => 16 + obj.iter().map(|(k, v)| k.len() + estimate_value_bytes(v)).sum::<usize>(),
        _ => 0,
    }
}

/// Check if a compiled node is static (can be evaluated without runtime context).
pub(crate) fn node_is_static(node: &CompiledNode) -> bool {
    match node {
        CompiledNode::Value { .. } => true,
        CompiledNode::Array { nodes, .. } => nodes.iter().all(node_is_static),
        CompiledNode::BuiltinOperator { opcode, args, .. } => opcode_is_static(opcode, args),
        CompiledNode::CustomOperator(_) => false,
        CompiledNode::CompiledVar { .. } => false,
        #[cfg(feature = "ext-control")]
        CompiledNode::CompiledExists(_) => false,
        #[cfg(feature = "ext-string")]
        CompiledNode::CompiledSplitRegex(data) => data.args.iter().all(node_is_static),
        #[cfg(feature = "error-handling")]
        CompiledNode::CompiledThrow(_) => false,
        #[cfg(feature = "preserve")]
        CompiledNode::StructuredObject(data) => {
            data.fields.iter().all(|(_, node)| node_is_static(node))
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
        Var | Missing | MissingSome => false,
        #[cfg(feature = "ext-control")]
        Val | Exists => false,

        // Iteration operators: These push new contexts for each iteration and use
        // callbacks that may reference the iteration variable. Even with static
        // arrays, the callback logic depends on the per-element context.
        Map | Filter | Reduce | All | Some | None => false,

        // Error handling: These have control flow effects (early exit, error propagation)
        // that should be preserved for runtime execution.
        #[cfg(feature = "error-handling")]
        Try | Throw => false,

        // Time-dependent: Returns current UTC time, inherently non-static.
        #[cfg(feature = "datetime")]
        Now => false,

        // Runtime disambiguation needed:
        // - Preserve: Must know it was explicitly used as an operator, not inferred
        // - Merge/Min/Max: Need to distinguish [1,2,3] literal from operator arguments
        //   at runtime to handle nested arrays correctly
        #[cfg(feature = "preserve")]
        Preserve => false,
        Merge | Min | Max => false,

        // Pure operators: Static when all arguments are static. These perform
        // deterministic transformations without side effects or context access.
        _ => args_static(),
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
        CompiledNode::CustomOperator(data) => {
            let mut obj = serde_json::Map::new();
            let args_value = if data.args.len() == 1 {
                node_to_value(&data.args[0])
            } else {
                Value::Array(data.args.iter().map(node_to_value).collect())
            };
            obj.insert(data.name.clone(), args_value);
            Value::Object(obj)
        }
        #[cfg(feature = "preserve")]
        CompiledNode::StructuredObject(data) => {
            let mut obj = serde_json::Map::new();
            for (key, node) in data.fields.iter() {
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
        #[cfg(feature = "ext-control")]
        CompiledNode::CompiledExists(data) => {
            let mut obj = serde_json::Map::new();
            if data.segments.len() == 1 {
                obj.insert("exists".into(), segment_to_value(&data.segments[0]));
            } else {
                let arr: Vec<Value> = data.segments.iter().map(segment_to_value).collect();
                obj.insert("exists".into(), Value::Array(arr));
            }
            Value::Object(obj)
        }
        #[cfg(feature = "ext-string")]
        CompiledNode::CompiledSplitRegex(data) => {
            let mut obj = serde_json::Map::new();
            let mut arr = vec![node_to_value(&data.args[0])];
            arr.push(Value::String(data.regex.as_str().to_string()));
            obj.insert("split".into(), Value::Array(arr));
            Value::Object(obj)
        }
        #[cfg(feature = "error-handling")]
        CompiledNode::CompiledThrow(data) => {
            let mut obj = serde_json::Map::new();
            if let Value::Object(err_map) = &data.error {
                if let Some(Value::String(s)) = err_map.get("type") {
                    obj.insert("throw".into(), Value::String(s.clone()));
                } else {
                    obj.insert("throw".into(), data.error.clone());
                }
            } else {
                obj.insert("throw".into(), data.error.clone());
            }
            Value::Object(obj)
        }
    }
}
