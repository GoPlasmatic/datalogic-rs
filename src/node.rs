use std::num::NonZeroU32;

use crate::arena::DataValue;
use crate::opcode::OpCode;
use datavalue::OwnedDataValue;

/// Compile-time id assigned to every [`CompiledNode`].
///
/// `Some(n)` for nodes produced by the compile pipeline (where the counter
/// starts at 1). `None` for synthetic nodes built outside the pipeline —
/// test helpers, optimizer literal-replacement folds, `eager_apply` value
/// wrappers — which are never observed by tracing or error reporting.
///
/// Encoding the synthetic case as `None` (rather than the previous
/// `u32 = 0`) lets the type system catch the "forgot to bump the counter"
/// bug at construction sites: `id: ctx.next_id()` no longer compiles
/// against `Option<NonZeroU32>`, forcing the writer to choose between
/// `Some(ctx.next_id())` (real) and `SYNTHETIC_ID` (synthetic).
pub(crate) type NodeId = Option<NonZeroU32>;

/// Pre-build a `DataValue<'static>` for primitive literals that don't
/// require additional storage (Numbers — inline `NumberValue`). Used at
/// `CompiledNode::Value` construction time so the arena dispatch hot path
/// can return a borrow without re-arena work for the most common
/// primitive case. Other literal shapes (Null/Bool/String/Array/Object)
/// are populated post-compile by [`populate_lits`] using the
/// per-`Logic` static arena.
#[inline]
fn precompute_lit(value: &OwnedDataValue) -> Option<Box<DataValue<'static>>> {
    match value {
        OwnedDataValue::Number(n) => Some(Box::new(DataValue::Number(*n))),
        _ => None,
    }
}

/// Build an `DataValue<'a>` from an [`OwnedDataValue`] using the supplied
/// arena, then transmute the lifetime to `'static`. Used by the post-compile
/// populate pass: the resulting `'static` claim is upheld by the caller
/// owning the arena alongside the references inside the same struct
/// ([`Logic`]).
///
/// # Safety
///
/// The returned `DataValue<'static>` borrows into `arena`. The caller must
/// ensure `arena` outlives every read of the returned value. In practice
/// this is upheld by storing the arena and the result in the same owning
/// struct (`Logic`) — the references can be accessed only through
/// `&Logic`, which keeps the arena alive for the access.
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

/// Opcodes that consume `args[0]` as an iterator input via
/// [`crate::operators::array::resolve_iter_input`]. Used by the post-compile
/// populate pass to decide whether `iter_arg_kind` should be classified or
/// left at the `General` default. Mirrors the actual call sites — Merge does
/// not currently route through `resolve_iter_input`.
#[inline]
fn iterates_args0(opcode: OpCode) -> bool {
    let _opcode = opcode;
    #[cfg(feature = "ext-array")]
    if matches!(opcode, OpCode::Sort) {
        return true;
    }
    matches!(
        opcode,
        OpCode::Filter
            | OpCode::Map
            | OpCode::All
            | OpCode::Some
            | OpCode::None
            | OpCode::Reduce
            | OpCode::Min
            | OpCode::Max
    )
}

/// Walk the compiled tree and populate `lit` for every literal whose
/// `lit` is currently `None` — this covers Null, Bool, String, Array,
/// and Object literals that [`precompute_lit`] left out at
/// construction time. Allocations land in the supplied `arena`, which must
/// be moved into the owning [`Logic`] alongside the modified tree.
///
/// Also populates `CompiledThrowData::precomputed_error` so the error-handling
/// path can return arena-native values without per-throw `value_to_data`.
///
/// # Safety
///
/// The populated `lit` / `precomputed_error` values borrow from `arena`
/// despite their `'static` type. The caller must keep `arena` alive at
/// least as long as the modified tree is accessible. See
/// [`build_static_arena_value`] for the underlying invariant.
pub(crate) unsafe fn populate_lits(node: &mut CompiledNode, arena: &bumpalo::Bump) {
    // Recurse into AST children first so per-variant local work below sees
    // any populated `lit` / `precomputed_error` already in place.
    node.visit_children_mut(&mut |child| unsafe { populate_lits(child, arena) });

    // Per-variant local work that doesn't recurse.
    match node {
        CompiledNode::Value { value, lit, .. } => {
            if lit.is_none() {
                let av = unsafe { build_static_arena_value(value, arena) };
                *lit = Some(Box::new(av));
            }
        }
        CompiledNode::BuiltinOperator {
            opcode,
            args,
            predicate_hint,
            iter_arg_kind,
            ..
        } => {
            // Cache the fast-predicate detection result so quantifier/filter
            // operators consult `predicate_hint` instead of re-running the
            // structural detection on every iteration. Re-derive on every
            // call (rather than guarding with `is_none`) so a clone of an
            // already-populated tree gets a fresh hint matching the cloned
            // args — `Box<[PathSegment]>` and `OwnedDataValue` move on clone,
            // and the cached hint borrows nothing from them anyway.
            *predicate_hint =
                crate::operators::array::FastPredicate::try_detect_owned(*opcode, args)
                    .map(Box::new);
            // Cache the iterator-input classification for ops that consume
            // `args[0]` as an iterable. Read by `resolve_iter_input` so the
            // runtime shape match collapses to a byte compare. Other opcodes
            // keep the default `General` (the populate pass overwrites on
            // every clone).
            *iter_arg_kind = if iterates_args0(*opcode) && !args.is_empty() {
                crate::operators::array::IterArgKind::classify(&args[0])
            } else {
                crate::operators::array::IterArgKind::General
            };
        }
        #[cfg(feature = "error-handling")]
        CompiledNode::Throw(data) => {
            if data.precomputed_error.is_none() {
                let av = unsafe { build_static_arena_value(&data.error, arena) };
                data.precomputed_error = Some(Box::new(av));
            }
        }
        // No local work — recursion above handled all children.
        _ => {}
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
/// (test helpers, run-time value wrappers in `eager_apply`, etc.). Real ids
/// are `Some(NonZeroU32)` since `CompileCtx` starts the counter at 1.
pub(crate) const SYNTHETIC_ID: NodeId = None;

/// Data for a custom operator (boxed inside CompiledNode to reduce enum size).
#[derive(Debug, Clone)]
pub struct CustomOperatorData {
    pub id: NodeId,
    pub name: String,
    pub args: Box<[CompiledNode]>,
}

/// Data for a structured object template (boxed inside CompiledNode to reduce enum size).
#[cfg(feature = "preserve")]
#[derive(Debug, Clone)]
pub struct StructuredObjectData {
    pub id: NodeId,
    pub fields: Box<[(String, CompiledNode)]>,
}

/// Data for a pre-compiled exists check (boxed inside CompiledNode to reduce enum size).
#[cfg(feature = "ext-control")]
#[derive(Debug, Clone)]
pub struct CompiledExistsData {
    pub id: NodeId,
    pub scope_level: u32,
    pub segments: Box<[PathSegment]>,
}

/// Two-stage value: either resolved at compile time (`Now(S)`) or carried
/// as a [`CompiledNode`] (`Later(D)`) to be evaluated against the runtime
/// context. Used by every spot in `missing` / `missing_some` compilation
/// where an arg can be a literal we can pre-parse or an expression that
/// must wait until evaluation.
#[derive(Debug, Clone)]
pub enum Resolved<S, D> {
    /// Compile-time value — pre-parsed / pre-computed during compilation.
    Now(S),
    /// Runtime expression — evaluate against the live context.
    Later(D),
}

/// Pre-parsed `(raw_path, segments)` pair — the compile-time form of a
/// `missing` / `missing_some` path argument.
pub type StaticMissingPath = (Box<str>, Box<[PathSegment]>);

/// One arg to a `missing` / `missing_some` operator. Literal string paths
/// are pre-parsed into segments at compile time so the runtime walks the
/// input data without re-splitting the string or BTreeMap-keying via a
/// borrowed `&str` on every call.
pub type CompiledMissingArg = Resolved<StaticMissingPath, CompiledNode>;

/// `missing_some` minimum-present argument. `Now(usize)` is a literal
/// integer resolved at compile time; `Later(_)` is a runtime expression.
pub type CompiledMissingMin = Resolved<usize, CompiledNode>;

/// `missing_some` paths argument. `Now(_)` is a literal array of pre-parsed
/// paths; `Later(_)` is a runtime expression returning an array.
pub type CompiledMissingPaths = Resolved<Box<[StaticMissingPath]>, CompiledNode>;

/// Data for a pre-compiled `missing` operator.
#[derive(Debug, Clone)]
pub struct CompiledMissingData {
    pub id: NodeId,
    pub args: Box<[CompiledMissingArg]>,
}

/// Data for a pre-compiled `missing_some` operator. `min_present` may be a
/// literal integer (resolved at compile time) or a runtime expression.
#[derive(Debug, Clone)]
pub struct CompiledMissingSomeData {
    pub id: NodeId,
    pub min_present: CompiledMissingMin,
    pub paths: CompiledMissingPaths,
}

/// Data for a pre-compiled throw with a static error object.
/// Previously `Box<Value>`; upgraded to a named struct so it can carry an id
/// alongside the error payload.
#[cfg(feature = "error-handling")]
#[derive(Debug, Clone)]
pub struct CompiledThrowData {
    pub id: NodeId,
    pub error: OwnedDataValue,
    /// Arena-resident mirror of `error` populated post-compile by
    /// [`populate_lits`]. The borrowed lifetime is `'static` only
    /// because the backing storage lives in [`Logic::static_arena`],
    /// which is moved into the same owning struct.
    #[doc(hidden)]
    pub(crate) precomputed_error: Option<Box<DataValue<'static>>>,
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
    /// `lit` holds a pre-built `DataValue` for primitive literals
    /// that don't borrow from a per-call arena (e.g. Number). The arena
    /// dispatch hot path returns this borrow directly, skipping
    /// `value_to_data` and the per-call `arena.alloc`. `None` for
    /// composite literals (Array/Object) and for primitives already
    /// covered by static singletons (Null/Bool/empty string/empty array).
    /// Read-only after compile — safe to share across threads via
    /// `Arc<Logic>`.
    Value {
        id: NodeId,
        value: OwnedDataValue,
        lit: Option<Box<DataValue<'static>>>,
    },

    /// An array of compiled nodes.
    ///
    /// Each node is evaluated in sequence, and the results are collected into a JSON array.
    /// Uses `Box<[CompiledNode]>` for memory efficiency.
    Array {
        id: NodeId,
        nodes: Box<[CompiledNode]>,
    },

    /// A built-in operator optimized with OpCode dispatch.
    ///
    /// The OpCode enum enables direct dispatch without string lookups,
    /// significantly improving performance for the 50+ built-in operators.
    ///
    /// `predicate_hint` caches the result of [`FastPredicate::try_detect_owned`]
    /// so quantifier/filter operators don't repeat the structural pattern
    /// match on every iteration. Populated post-compile by
    /// [`populate_lits`]; `None` for nodes that aren't a fast-predicate
    /// shape, and re-derived after every clone (the populate pass overwrites
    /// the field).
    ///
    /// `iter_arg_kind` caches the
    /// [`crate::operators::array::IterArgKind::classify`] result for `args[0]`
    /// when this op iterates (filter/map/all/some/none/reduce/merge/min/max).
    /// `IterArgKind::General` for everything else — the dispatcher reads the
    /// kind and forwards it to `resolve_iter_input`, sidestepping the per-call
    /// pattern match on the iterator input's shape. Re-derived on every
    /// populate-arena-lits pass so clones stay correct.
    BuiltinOperator {
        id: NodeId,
        opcode: OpCode,
        args: Box<[CompiledNode]>,
        predicate_hint: Option<Box<crate::operators::array::FastPredicate>>,
        iter_arg_kind: crate::operators::array::IterArgKind,
    },

    /// A custom operator registered via `Engine::add_operator`.
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
    Var {
        id: NodeId,
        scope_level: u32,
        segments: Box<[PathSegment]>,
        reduce_hint: ReduceHint,
        metadata_hint: MetadataHint,
        default_value: Option<Box<CompiledNode>>,
    },

    /// A pre-compiled exists check.
    /// Boxed to reduce enum size (rare variant).
    #[cfg(feature = "ext-control")]
    Exists(Box<CompiledExistsData>),

    /// A pre-compiled throw with a static error object.
    /// Boxed to reduce enum size (rare variant).
    #[cfg(feature = "error-handling")]
    Throw(Box<CompiledThrowData>),

    /// A pre-compiled `missing` operator with paths parsed into segments.
    Missing(Box<CompiledMissingData>),

    /// A pre-compiled `missing_some` operator with paths parsed into segments
    /// and (where literal) min-count resolved.
    MissingSome(Box<CompiledMissingSomeData>),
}

impl CompiledNode {
    /// Returns the unique id assigned to this node during compilation, as
    /// the public `u32` shape used by trace/error breadcrumbs (`0` for
    /// synthetic nodes, matching the previous sentinel).
    ///
    /// IDs are shared across tracing and error breadcrumbs — one source of
    /// truth per node. Synthetic nodes built outside the compile pipeline
    /// (test helpers, `eager_apply` value wrappers) carry [`SYNTHETIC_ID`]
    /// and round-trip to `0` here.
    #[inline]
    pub fn id(&self) -> u32 {
        self.node_id().map(NonZeroU32::get).unwrap_or(0)
    }

    /// Returns the structured node id (real `Some(NonZero)` vs. synthetic
    /// `None`). Internal callers prefer this over [`Self::id`] when they
    /// need to distinguish the two cases.
    #[inline]
    pub(crate) fn node_id(&self) -> NodeId {
        match self {
            CompiledNode::Value { id, .. } => *id,
            CompiledNode::Array { id, .. } => *id,
            CompiledNode::BuiltinOperator { id, .. } => *id,
            CompiledNode::CustomOperator(data) => data.id,
            #[cfg(feature = "preserve")]
            CompiledNode::StructuredObject(data) => data.id,
            CompiledNode::Var { id, .. } => *id,
            #[cfg(feature = "ext-control")]
            CompiledNode::Exists(data) => data.id,
            #[cfg(feature = "error-handling")]
            CompiledNode::Throw(data) => data.id,
            CompiledNode::Missing(data) => data.id,
            CompiledNode::MissingSome(data) => data.id,
        }
    }

    /// Invoke `f` on each AST child of `self`, in JSONLogic-positional
    /// order, paired with the child's positional index (matching
    /// [`crate::PathStep::arg_index`] semantics).
    ///
    /// Single source of truth for "what are this node's children" — the
    /// post-compile populate pass, the static-byte estimator, and the
    /// path resolver all defer to this rather than pattern-matching every
    /// variant themselves.
    pub(crate) fn visit_indexed_children<F: FnMut(u32, &CompiledNode)>(&self, f: &mut F) {
        match self {
            CompiledNode::Value { .. } => {}
            CompiledNode::Array { nodes, .. } => {
                for (i, n) in nodes.iter().enumerate() {
                    f(i as u32, n);
                }
            }
            CompiledNode::BuiltinOperator { args, .. } => {
                for (i, n) in args.iter().enumerate() {
                    f(i as u32, n);
                }
            }
            CompiledNode::CustomOperator(data) => {
                for (i, n) in data.args.iter().enumerate() {
                    f(i as u32, n);
                }
            }
            #[cfg(feature = "preserve")]
            CompiledNode::StructuredObject(data) => {
                for (i, (_, n)) in data.fields.iter().enumerate() {
                    f(i as u32, n);
                }
            }
            CompiledNode::Var { default_value, .. } => {
                if let Some(d) = default_value {
                    f(0, d);
                }
            }
            #[cfg(feature = "ext-control")]
            CompiledNode::Exists(_) => {}
            #[cfg(feature = "error-handling")]
            CompiledNode::Throw(_) => {}
            CompiledNode::Missing(data) => {
                for (i, arg) in data.args.iter().enumerate() {
                    if let CompiledMissingArg::Later(n) = arg {
                        f(i as u32, n);
                    }
                }
            }
            CompiledNode::MissingSome(data) => {
                if let CompiledMissingMin::Later(n) = &data.min_present {
                    f(0, n);
                }
                if let CompiledMissingPaths::Later(n) = &data.paths {
                    f(1, n);
                }
            }
        }
    }

    /// Convenience wrapper over [`Self::visit_indexed_children`] for callers
    /// that don't care about the positional index.
    #[inline]
    pub(crate) fn visit_children<F: FnMut(&CompiledNode)>(&self, f: &mut F) {
        self.visit_indexed_children(&mut |_, child| f(child));
    }

    /// Mutable mirror of [`Self::visit_children`]. Used by the post-compile
    /// populate pass to walk the tree once for both per-variant local work
    /// and recursion.
    pub(crate) fn visit_children_mut<F: FnMut(&mut CompiledNode)>(&mut self, f: &mut F) {
        match self {
            CompiledNode::Value { .. } => {}
            CompiledNode::Array { nodes, .. } => {
                for n in nodes.iter_mut() {
                    f(n);
                }
            }
            CompiledNode::BuiltinOperator { args, .. } => {
                for n in args.iter_mut() {
                    f(n);
                }
            }
            CompiledNode::CustomOperator(data) => {
                for n in data.args.iter_mut() {
                    f(n);
                }
            }
            #[cfg(feature = "preserve")]
            CompiledNode::StructuredObject(data) => {
                for (_, n) in data.fields.iter_mut() {
                    f(n);
                }
            }
            CompiledNode::Var { default_value, .. } => {
                if let Some(d) = default_value {
                    f(d);
                }
            }
            #[cfg(feature = "ext-control")]
            CompiledNode::Exists(_) => {}
            #[cfg(feature = "error-handling")]
            CompiledNode::Throw(_) => {}
            CompiledNode::Missing(data) => {
                for arg in data.args.iter_mut() {
                    if let CompiledMissingArg::Later(n) = arg {
                        f(n);
                    }
                }
            }
            CompiledNode::MissingSome(data) => {
                if let CompiledMissingMin::Later(n) = &mut data.min_present {
                    f(n);
                }
                if let CompiledMissingPaths::Later(n) = &mut data.paths {
                    f(n);
                }
            }
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
    /// the precomputed `lit` for primitive literals so the arena
    /// dispatch hot path can borrow it without a per-call `arena.alloc`.
    /// Centralised here so every construction site stays in sync — adding
    /// a new precomputable variant only requires editing
    /// [`precompute_lit`].
    #[inline]
    pub fn value_with_id(id: NodeId, value: OwnedDataValue) -> Self {
        let lit = precompute_lit(&value);
        CompiledNode::Value { id, value, lit }
    }

    /// Returns the name of this node's top-level operator, if any.
    ///
    /// Used when wrapping an error with structured context — we only report
    /// the outermost operator, not the full nested call chain.
    pub fn operator_name(&self) -> Option<String> {
        match self {
            CompiledNode::BuiltinOperator { opcode, .. } => Some(opcode.as_str().to_string()),
            CompiledNode::CustomOperator(data) => Some(data.name.clone()),
            CompiledNode::Var { .. } => Some("var".to_string()),
            #[cfg(feature = "ext-control")]
            CompiledNode::Exists(_) => Some("exists".to_string()),
            #[cfg(feature = "error-handling")]
            CompiledNode::Throw(_) => Some("throw".to_string()),
            CompiledNode::Missing(_) => Some("missing".to_string()),
            CompiledNode::MissingSome(_) => Some("missing_some".to_string()),
            _ => None,
        }
    }
}

/// Compile-time context for assigning unique node ids and threading the
/// "skip optimization" flag through the recursive descent.
///
/// `next_id` ensures every node constructed during compilation gets a fresh,
/// monotonically increasing id. The counter is [`NonZeroU32`] starting at 1;
/// the synthetic case is encoded as `None` (see [`SYNTHETIC_ID`]) and never
/// flows through this counter.
///
/// `skip_fold` is set by the trace path so the constant-fold + optimizer
/// passes are bypassed and every operator survives in the compiled tree.
#[derive(Debug)]
pub(crate) struct CompileCtx {
    next_id: NonZeroU32,
    skip_fold: bool,
}

const ID_ONE: NonZeroU32 = match NonZeroU32::new(1) {
    Some(n) => n,
    None => unreachable!(),
};

impl CompileCtx {
    pub(crate) fn new() -> Self {
        Self {
            next_id: ID_ONE,
            skip_fold: false,
        }
    }

    /// Construct a context that skips the optimizer + constant-fold passes.
    /// Used by the internal trace compile path so traced rules retain every
    /// operator as a step source.
    #[cfg(feature = "trace")]
    pub(crate) fn no_fold() -> Self {
        Self {
            next_id: ID_ONE,
            skip_fold: true,
        }
    }

    /// Allocate a fresh node id. Returns the bare [`NonZeroU32`] — callers
    /// wrap it in `Some(...)` at the construction site, making the
    /// real-vs-synthetic choice explicit and forcing a type error if the
    /// id field is left unassigned.
    #[inline]
    pub(crate) fn next_id(&mut self) -> NonZeroU32 {
        let id = self.next_id;
        self.next_id = self.next_id.saturating_add(1);
        id
    }

    /// Whether to skip the optimizer + constant-fold passes during compile.
    #[inline]
    pub(crate) fn skip_fold(&self) -> bool {
        self.skip_fold
    }
}

/// Compiled logic that can be evaluated multiple times across different data.
///
/// `Logic` represents a pre-processed JSONLogic expression that has been
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
/// use std::sync::Arc;
/// use datalogic_rs::Engine;
///
/// let engine = Engine::new();
/// let compiled = engine.compile_arc(r#"{">": [{"var": "score"}, 90]}"#).unwrap();
///
/// // Compiled logic can be cloned cheaply (atomic refcount) and sent across threads.
/// let compiled_clone = Arc::clone(&compiled);
/// std::thread::spawn(move || {
///     let engine = Engine::new();
///     let _result = engine
///         .scratch()
///         .eval_str(&compiled_clone, r#"{"score": 95}"#)
///         .unwrap();
/// });
/// ```
pub struct Logic {
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
    /// Per-`Logic` arena that backs `lit` storage on every
    /// literal `CompiledNode::Value` and `CompiledThrowData::precomputed_error`
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
// `Bump::alloc` take `&self` and mutate internal chunk state. `Logic`
// only ever allocates into `static_arena` during construction (see
// [`Logic::new`]). After construction, no method on `&Logic`
// reaches into `static_arena` — the arena is read-only via the existing
// `&'static DataValue<'static>` references stored in `root`. Concurrent
// `&Logic` readers therefore never race on `Bump`'s internal cells.
unsafe impl Sync for Logic {}

impl std::fmt::Debug for Logic {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Logic")
            .field("root", &self.root)
            .field("arena_static_bytes", &self.arena_static_bytes)
            .finish_non_exhaustive()
    }
}

impl Logic {
    /// Creates a new compiled logic from a root node.
    ///
    /// Allocates the per-`Logic` static arena, sized to the
    /// conservative estimate, and runs the post-compile populate pass to
    /// fill in `lit` for every literal node and `precomputed_error` on
    /// throw nodes. After this call, the arena is logically frozen.
    ///
    /// # Arguments
    ///
    /// * `root` - The root node of the compiled logic tree
    pub fn new(mut root: CompiledNode) -> Self {
        let arena_static_bytes = estimate_arena_static_bytes(&root);
        let static_arena = bumpalo::Bump::with_capacity(arena_static_bytes);
        // SAFETY: `static_arena` is moved into `Self` together with `root`.
        // The `'static`-typed references that `populate_lits` plants
        // inside `root` actually borrow from `static_arena`; both are owned
        // by the same struct, so the references stay valid for as long as
        // `Self` is accessible. After this call, nothing else allocates into
        // `static_arena`, satisfying the [`Sync`] invariant above.
        unsafe {
            populate_lits(&mut root, &static_arena);
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
}

/// Estimate the static (rule-dependent, data-independent) portion of arena
/// bytes this rule will need at evaluation time. Conservative — overestimating
/// is harmless (one larger bumpalo chunk), underestimating costs an extra
/// chunk allocation. Data-dependent allocations (filter results, map outputs)
/// can't be predicted here.
fn estimate_arena_static_bytes(node: &CompiledNode) -> usize {
    // Base cost per node when promoted to `DataValue`: the enum itself plus
    // a slice-header fudge for nodes whose payload lives as `&[…]` in the
    // arena (Array, Object, structured-object fields). The DataValue-size
    // term tracks layout changes automatically — without datetime it's
    // typically 24 bytes (8-byte discriminant + 16-byte fat-pointer
    // payload), with datetime it grows to fit `DataDateTime`. String
    // content for literals is added separately by `estimate_value_bytes`.
    const PER_NODE: usize =
        std::mem::size_of::<DataValue<'static>>() + std::mem::size_of::<&[u8]>();
    let mut bytes = PER_NODE;

    // Per-variant size contributions that aren't covered by recursing into
    // AST children (literal payloads, structured-object key strings, etc.).
    match node {
        CompiledNode::Value { value, .. } => {
            bytes += estimate_value_bytes(value);
        }
        #[cfg(feature = "error-handling")]
        CompiledNode::Throw(data) => {
            bytes += estimate_value_bytes(&data.error);
        }
        #[cfg(feature = "preserve")]
        CompiledNode::StructuredObject(data) => {
            for (k, _) in data.fields.iter() {
                bytes += k.len();
            }
        }
        _ => {}
    }

    // Recurse into AST children via the shared visitor — single source of
    // truth for "what are this node's children".
    node.visit_children(&mut |child| {
        bytes += estimate_arena_static_bytes(child);
    });

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
        CompiledNode::Var { .. } => false,
        #[cfg(feature = "ext-control")]
        CompiledNode::Exists(_) => false,
        #[cfg(feature = "error-handling")]
        CompiledNode::Throw(_) => false,
        #[cfg(feature = "preserve")]
        CompiledNode::StructuredObject(data) => {
            data.fields.iter().all(|(_, node)| node_is_static(node))
        }
        CompiledNode::Missing(_) | CompiledNode::MissingSome(_) => false,
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
/// 5. Needs runtime disambiguation (`merge`, `min`, `max`)
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

        // Runtime disambiguation needed: Merge/Min/Max have to distinguish
        // a [1,2,3] literal from operator arguments at runtime to handle
        // nested arrays correctly.
        Merge | Min | Max => false,

        // Pure operators: Static when all arguments are static. These perform
        // deterministic transformations without side effects or context access.
        _ => args_static(),
    }
}
