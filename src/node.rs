use crate::arena::DataValue;
use crate::opcode::OpCode;
use datavalue::OwnedDataValue;

/// Pre-build a `DataValue<'static>` for primitive literals that don't
/// require additional storage (Numbers — inline `NumberValue`). Used at
/// `CompiledNode::Value` construction time so the arena dispatch hot path
/// can return a borrow without re-arena work for the most common
/// primitive case. Other literal shapes (Null/Bool/String/Array/Object)
/// are populated post-compile by [`populate_arena_lits`] using the
/// per-`CompiledLogic` static arena.
#[inline]
fn precompute_arena_lit(value: &OwnedDataValue) -> Option<Box<DataValue<'static>>> {
    match value {
        OwnedDataValue::Number(n) => Some(Box::new(DataValue::Number(*n))),
        _ => None,
    }
}

/// Build an `DataValue<'a>` from an [`OwnedDataValue`] using the supplied
/// arena, then transmute the lifetime to `'static`. Used by the post-compile
/// populate pass: the resulting `'static` claim is upheld by the caller
/// owning the arena alongside the references inside the same struct
/// ([`CompiledLogic`]).
///
/// # Safety
///
/// The returned `DataValue<'static>` borrows into `arena`. The caller must
/// ensure `arena` outlives every read of the returned value. In practice
/// this is upheld by storing the arena and the result in the same owning
/// struct (`CompiledLogic`) — the references can be accessed only through
/// `&CompiledLogic`, which keeps the arena alive for the access.
#[inline]
unsafe fn build_static_arena_value(
    value: &OwnedDataValue,
    arena: &bumpalo::Bump,
) -> DataValue<'static> {
    let av = value.to_arena(arena);
    // SAFETY: caller guarantees `arena` lives at least as long as any read of
    // the returned `'static` value. Layout-identical lifetime cast.
    unsafe { core::mem::transmute::<DataValue<'_>, DataValue<'static>>(av) }
}

/// Walk the compiled tree and populate `arena_lit` for every literal whose
/// `arena_lit` is currently `None` — this covers Null, Bool, String, Array,
/// and Object literals that [`precompute_arena_lit`] left out at
/// construction time. Allocations land in the supplied `arena`, which must
/// be moved into the owning [`CompiledLogic`] alongside the modified tree.
///
/// Also populates `CompiledThrowData::arena_error` so the error-handling
/// path can return arena-native values without per-throw `value_to_arena`.
///
/// # Safety
///
/// The populated `arena_lit` / `arena_error` values borrow from `arena`
/// despite their `'static` type. The caller must keep `arena` alive at
/// least as long as the modified tree is accessible. See
/// [`build_static_arena_value`] for the underlying invariant.
pub(crate) unsafe fn populate_arena_lits(node: &mut CompiledNode, arena: &bumpalo::Bump) {
    match node {
        CompiledNode::Value {
            value, arena_lit, ..
        } => {
            if arena_lit.is_none() {
                let av = unsafe { build_static_arena_value(value, arena) };
                *arena_lit = Some(Box::new(av));
            }
        }
        CompiledNode::Array { nodes, .. } => {
            for n in nodes.iter_mut() {
                unsafe { populate_arena_lits(n, arena) };
            }
        }
        CompiledNode::BuiltinOperator { args, .. } => {
            for n in args.iter_mut() {
                unsafe { populate_arena_lits(n, arena) };
            }
        }
        CompiledNode::CustomOperator(data) => {
            for n in data.args.iter_mut() {
                unsafe { populate_arena_lits(n, arena) };
            }
        }
        #[cfg(feature = "preserve")]
        CompiledNode::StructuredObject(data) => {
            for (_, n) in data.fields.iter_mut() {
                unsafe { populate_arena_lits(n, arena) };
            }
        }
        CompiledNode::CompiledVar { default_value, .. } => {
            if let Some(d) = default_value {
                unsafe { populate_arena_lits(d, arena) };
            }
        }
        #[cfg(feature = "ext-control")]
        CompiledNode::CompiledExists(_) => {}
        #[cfg(feature = "error-handling")]
        CompiledNode::CompiledThrow(data) => {
            if data.arena_error.is_none() {
                let av = unsafe { build_static_arena_value(&data.error, arena) };
                data.arena_error = Some(Box::new(av));
            }
        }
        CompiledNode::CompiledMissing(data) => {
            for arg in data.args.iter_mut() {
                if let CompiledMissingArg::Dynamic(n) = arg {
                    unsafe { populate_arena_lits(n, arena) };
                }
            }
        }
        CompiledNode::CompiledMissingSome(data) => {
            if let CompiledMissingMin::Dynamic(n) = &mut data.min_present {
                unsafe { populate_arena_lits(n, arena) };
            }
            if let CompiledMissingPaths::Dynamic(n) = &mut data.paths {
                unsafe { populate_arena_lits(n, arena) };
            }
        }
    }
}

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
pub(crate) const SYNTHETIC_ID: u32 = 0;

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

/// One arg to a `missing` / `missing_some` operator. Static literal paths are
/// pre-parsed into segments at compile time so the runtime walks the input
/// data without re-splitting the string or BTreeMap-keying via a borrowed
/// `&str` on every call.
#[derive(Debug, Clone)]
pub enum CompiledMissingArg {
    /// Literal string path resolved at compile time. `path` is the original
    /// string to emit when the lookup fails; `segments` is its parse.
    Static {
        path: Box<str>,
        segments: Box<[PathSegment]>,
    },
    /// Anything else (literal arrays-of-strings, expressions, var lookups…) —
    /// evaluated at runtime, results coerced to path string(s) as before.
    Dynamic(CompiledNode),
}

/// Data for a pre-compiled `missing` operator.
#[derive(Debug, Clone)]
pub struct CompiledMissingData {
    pub id: u32,
    pub args: Box<[CompiledMissingArg]>,
}

/// Data for a pre-compiled `missing_some` operator. `min_present` may be a
/// literal integer (resolved at compile time) or a runtime expression.
#[derive(Debug, Clone)]
pub struct CompiledMissingSomeData {
    pub id: u32,
    pub min_present: CompiledMissingMin,
    pub paths: CompiledMissingPaths,
}

#[derive(Debug, Clone)]
pub enum CompiledMissingMin {
    Static(usize),
    Dynamic(CompiledNode),
}

/// Pre-parsed `(raw_path, segments)` pair — the static form of a
/// `missing_some` path argument.
pub type StaticMissingPath = (Box<str>, Box<[PathSegment]>);

#[derive(Debug, Clone)]
pub enum CompiledMissingPaths {
    /// Literal array of strings — every entry pre-parsed.
    Static(Box<[StaticMissingPath]>),
    /// Runtime expression returning an array.
    Dynamic(CompiledNode),
}

/// Data for a pre-compiled throw with a static error object.
/// Previously `Box<Value>`; upgraded to a named struct so it can carry an id
/// alongside the error payload.
#[cfg(feature = "error-handling")]
#[derive(Debug, Clone)]
pub struct CompiledThrowData {
    pub id: u32,
    pub error: OwnedDataValue,
    /// Arena-resident mirror of `error` populated post-compile by
    /// [`populate_arena_lits`]. The borrowed lifetime is `'static` only
    /// because the backing storage lives in [`CompiledLogic::static_arena`],
    /// which is moved into the same owning struct.
    #[doc(hidden)]
    pub(crate) arena_error: Option<Box<DataValue<'static>>>,
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
    ///
    /// `arena_lit` holds a pre-built `DataValue` for primitive literals
    /// that don't borrow from a per-call arena (e.g. Number). The arena
    /// dispatch hot path returns this borrow directly, skipping
    /// `value_to_arena` and the per-call `arena.alloc`. `None` for
    /// composite literals (Array/Object) and for primitives already
    /// covered by static singletons (Null/Bool/empty string/empty array).
    /// Read-only after compile — safe to share across threads via
    /// `Arc<CompiledLogic>`.
    Value {
        id: u32,
        value: OwnedDataValue,
        arena_lit: Option<Box<DataValue<'static>>>,
    },

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

    /// A pre-compiled throw with a static error object.
    /// Boxed to reduce enum size (rare variant).
    #[cfg(feature = "error-handling")]
    CompiledThrow(Box<CompiledThrowData>),

    /// A pre-compiled `missing` operator with paths parsed into segments.
    CompiledMissing(Box<CompiledMissingData>),

    /// A pre-compiled `missing_some` operator with paths parsed into segments
    /// and (where literal) min-count resolved.
    CompiledMissingSome(Box<CompiledMissingSomeData>),
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
            #[cfg(feature = "error-handling")]
            CompiledNode::CompiledThrow(data) => data.id,
            CompiledNode::CompiledMissing(data) => data.id,
            CompiledNode::CompiledMissingSome(data) => data.id,
        }
    }

    /// Convenience constructor for a `Value` node with a [`SYNTHETIC_ID`].
    ///
    /// Used by operator fast paths that wrap runtime values back into
    /// `CompiledNode::Value` purely for dispatch. These wrappers are never
    /// observed by tracing or error reporting, so assigning a real id would
    /// be misleading.
    #[inline]
    pub fn synthetic_value(value: OwnedDataValue) -> Self {
        Self::value_with_id(SYNTHETIC_ID, value)
    }

    /// Construct a `CompiledNode::Value` with `id` and `value`, populating
    /// the precomputed `arena_lit` for primitive literals so the arena
    /// dispatch hot path can borrow it without a per-call `arena.alloc`.
    /// Centralised here so every construction site stays in sync — adding
    /// a new precomputable variant only requires editing
    /// [`precompute_arena_lit`].
    #[inline]
    pub fn value_with_id(id: u32, value: OwnedDataValue) -> Self {
        let arena_lit = precompute_arena_lit(&value);
        CompiledNode::Value {
            id,
            value,
            arena_lit,
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
            #[cfg(feature = "error-handling")]
            CompiledNode::CompiledThrow(_) => Some("throw".to_string()),
            CompiledNode::CompiledMissing(_) => Some("missing".to_string()),
            CompiledNode::CompiledMissingSome(_) => Some("missing_some".to_string()),
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
pub struct CompiledLogic {
    /// The root node of the compiled logic tree.
    ///
    /// Some `CompiledNode::Value` and `CompiledThrowData` nodes inside this
    /// tree carry `'static`-typed arena references that actually borrow from
    /// [`Self::static_arena`]. This is only sound because both fields are
    /// owned together by the same struct; never mutate `static_arena` after
    /// construction, and never extract `root` out of `Self`.
    pub root: CompiledNode,
    /// Conservative upper bound on the static portion of arena allocations
    /// this rule will need (literals, structured-object skeletons, etc.).
    /// Used to size the per-call `Bump` so the first chunk is large enough.
    /// `pub(crate)` — internal arena infrastructure.
    pub(crate) arena_static_bytes: usize,
    /// Per-`CompiledLogic` arena that backs `arena_lit` storage on every
    /// literal `CompiledNode::Value` and `CompiledThrowData::arena_error`
    /// inside `root`. Allocated and populated once during construction; never
    /// mutated afterward, which is what makes the [`Sync`] impl below sound
    /// despite `bumpalo::Bump` itself being `!Sync` (its allocation methods
    /// take `&self` and use interior mutability).
    ///
    /// Field is held purely to keep its allocations alive — the references
    /// into it live inside `root`. Reads happen via those references, not
    /// directly through this field, hence the `#[allow(dead_code)]`.
    #[allow(dead_code)]
    static_arena: bumpalo::Bump,
}

// SAFETY: `bumpalo::Bump` is `!Sync` because allocation methods like
// `Bump::alloc` take `&self` and mutate internal chunk state. `CompiledLogic`
// only ever allocates into `static_arena` during construction (see
// [`CompiledLogic::new`]). After construction, no method on `&CompiledLogic`
// reaches into `static_arena` — the arena is read-only via the existing
// `&'static DataValue<'static>` references stored in `root`. Concurrent
// `&CompiledLogic` readers therefore never race on `Bump`'s internal cells.
unsafe impl Sync for CompiledLogic {}

impl std::fmt::Debug for CompiledLogic {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("CompiledLogic")
            .field("root", &self.root)
            .field("arena_static_bytes", &self.arena_static_bytes)
            .finish_non_exhaustive()
    }
}

impl CompiledLogic {
    /// Creates a new compiled logic from a root node.
    ///
    /// Allocates the per-`CompiledLogic` static arena, sized to the
    /// conservative estimate, and runs the post-compile populate pass to
    /// fill in `arena_lit` for every literal node and `arena_error` on
    /// throw nodes. After this call, the arena is logically frozen.
    ///
    /// # Arguments
    ///
    /// * `root` - The root node of the compiled logic tree
    pub fn new(mut root: CompiledNode) -> Self {
        let arena_static_bytes = estimate_arena_static_bytes(&root);
        let static_arena = bumpalo::Bump::with_capacity(arena_static_bytes);
        // SAFETY: `static_arena` is moved into `Self` together with `root`.
        // The `'static`-typed references that `populate_arena_lits` plants
        // inside `root` actually borrow from `static_arena`; both are owned
        // by the same struct, so the references stay valid for as long as
        // `Self` is accessible. After this call, nothing else allocates into
        // `static_arena`, satisfying the [`Sync`] invariant above.
        unsafe {
            populate_arena_lits(&mut root, &static_arena);
        }
        Self {
            root,
            arena_static_bytes,
            static_arena,
        }
    }

    /// Check if this compiled logic is static (can be evaluated without context)
    pub fn is_static(&self) -> bool {
        node_is_static(&self.root)
    }

    /// Conservative arena capacity for one evaluation of this rule:
    /// `static_bytes × 2`, with a 4 KiB floor.
    #[cfg(feature = "compat")]
    #[inline]
    pub(crate) fn arena_capacity(&self) -> usize {
        self.arena_static_bytes.saturating_mul(2).max(4096)
    }
}

/// Estimate the static (rule-dependent, data-independent) portion of arena
/// bytes this rule will need at evaluation time. Conservative — overestimating
/// is harmless (one larger bumpalo chunk), underestimating costs an extra
/// chunk allocation. Data-dependent allocations (filter results, map outputs)
/// can't be predicted here.
fn estimate_arena_static_bytes(node: &CompiledNode) -> usize {
    // Base cost per node when promoted to DataValue: ~32 bytes for the enum +
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
        CompiledNode::CompiledMissing(data) => {
            for arg in data.args.iter() {
                if let CompiledMissingArg::Dynamic(n) = arg {
                    bytes += estimate_arena_static_bytes(n);
                }
            }
        }
        CompiledNode::CompiledMissingSome(data) => {
            if let CompiledMissingMin::Dynamic(n) = &data.min_present {
                bytes += estimate_arena_static_bytes(n);
            }
            if let CompiledMissingPaths::Dynamic(n) = &data.paths {
                bytes += estimate_arena_static_bytes(n);
            }
        }
    }
    bytes
}

fn estimate_value_bytes(v: &OwnedDataValue) -> usize {
    match v {
        OwnedDataValue::String(s) => s.len() + 16,
        OwnedDataValue::Array(arr) => 16 + arr.iter().map(estimate_value_bytes).sum::<usize>(),
        OwnedDataValue::Object(pairs) => {
            16 + pairs
                .iter()
                .map(|(k, v)| k.len() + estimate_value_bytes(v))
                .sum::<usize>()
        }
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
        #[cfg(feature = "error-handling")]
        CompiledNode::CompiledThrow(_) => false,
        #[cfg(feature = "preserve")]
        CompiledNode::StructuredObject(data) => {
            data.fields.iter().all(|(_, node)| node_is_static(node))
        }
        CompiledNode::CompiledMissing(_) | CompiledNode::CompiledMissingSome(_) => false,
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
fn opcode_is_static(opcode: &OpCode, args: &[CompiledNode]) -> bool {
    use OpCode::*;

    // Check if all arguments are static first (common pattern)
    let args_static = || args.iter().all(node_is_static);

    match opcode {
        // Context-dependent: These operators read from the data context, which is
        // not available at compile time. They must remain dynamic.
        Val | Missing | MissingSome => false,
        #[cfg(feature = "ext-control")]
        Exists => false,

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
