//! Boxed payload types and small helper enums referenced by
//! [`super::CompiledNode`]. Split out so the enum file stays focused on the
//! variant list and dispatch helpers.

use super::CompiledNode;
use super::compile_ctx::NodeId;
#[cfg(feature = "error-handling")]
use datavalue::OwnedDataValue;

/// A pre-parsed path segment for compiled variable access.
///
/// `PartialEq`/`Hash` are structural — used by the CSE pass to compare
/// `Var`/`Exists` subtrees for slot sharing.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub(crate) enum PathSegment {
    /// Object field access by key
    Field(Box<str>),
    /// Array element access by index
    Index(usize),
    /// Try as object key first, then as array index (for segments that could be either).
    /// Pre-parses the index at compile time to avoid runtime parsing.
    FieldOrIndex(Box<str>, usize),
}

/// Hint for reduce context resolution, detected at compile time.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub(crate) enum ReduceHint {
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
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub(crate) enum MetadataHint {
    /// Normal data access
    None,
    /// Access frame index metadata
    Index,
    /// Access frame key metadata
    Key,
}

/// Data for a custom operator (boxed inside CompiledNode to reduce enum size).
#[derive(Debug, Clone)]
pub(crate) struct CustomOperatorData {
    pub id: NodeId,
    pub name: String,
    pub args: Box<[CompiledNode]>,
}

/// Data for a CSE memo wrapper (boxed inside CompiledNode to reduce enum
/// size). Produced only by the compile-time CSE pass
/// (`crate::compile::optimize::cse`); `slot` indexes the per-evaluation
/// memo table on `ContextStack`, shared by every occurrence of one
/// equivalence class. Carries no id of its own — id, name, serialization,
/// and trace queries all delegate to `inner`.
#[derive(Debug, Clone)]
pub(crate) struct CseData {
    pub slot: u16,
    pub inner: CompiledNode,
}

/// Data for a structured object template (boxed inside CompiledNode to reduce enum size).
#[cfg(feature = "templating")]
#[derive(Debug, Clone)]
pub(crate) struct StructuredObjectData {
    pub id: NodeId,
    /// Field keys as they appear **in the source rule**, escape prefix
    /// included. `evaluate_structured_object` strips the prefix on the way
    /// out; keeping the source form here is what lets `to_json` round-trip
    /// (a stored bare `type` would re-parse as the `type` operator).
    pub fields: Box<[(String, CompiledNode)]>,
    /// Whether any key in `fields` carries the engine's escape prefix.
    /// Two jobs: it gates the per-key strip at evaluation time so
    /// unescaped templates pay nothing, and it makes the node non-static
    /// so constant folding can't collapse it into an object literal whose
    /// keys have already lost their escape.
    pub has_escaped_keys: bool,
}

/// Compile-time resolution of *which context frame* a variable reference
/// reads, computed by [`crate::compile::scope::resolve`] from the node's
/// static frame depth `D` and its `scope_level` `L`.
///
/// This is a predicate *over* [`crate::arena::ContextStack::get_at_level`],
/// never a second implementation of it: both call
/// [`crate::arena::frame_target`] for the arithmetic, so the pass decides
/// whether the walk can be skipped, and [`Self::Ancestor`] hands the job
/// back untouched. The mapping therefore matches the runtime exactly,
/// **off-by-one included** — `L == 1` at `D >= 2` reads the *current*
/// frame, not its parent.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub(crate) enum ScopeBinding {
    /// The pass has not run on this node — it was built outside the compile
    /// pipeline (optimizer test fixtures, runtime wrappers). The runtime
    /// falls back to the `scope_level` walk.
    ///
    /// This is the safety net that makes the whole change non-breaking: a
    /// node the pass fails to reach degrades to today's behaviour, never to
    /// a wrong frame. That is why the default is not `Root` — an unvisited
    /// node must not silently claim to read the rule input.
    #[default]
    Unresolved,
    /// Provably the rule's root input: `L == 0 && D == 0`, or `L >= D >= 1`
    /// (the clamp). Reads `ctx.root_input()` with no stack access.
    Root,
    /// Provably the innermost pushed frame, which is guaranteed to exist:
    /// `L == 0 && D > 0`, or `L == 1 && D >= 2` (the off-by-one).
    /// Reads `ctx.current()` with no depth probe and no clamp test.
    Current,
    /// A strict ancestor frame (`2 <= L <= D - 1`, so `D >= 3`). The pass
    /// proves only that the clamp cannot fire; the runtime still walks.
    Ancestor,
}

impl ScopeBinding {
    /// Resolve a `(static_depth, scope_level)` pair with the same
    /// arithmetic the runtime walk uses.
    pub(crate) fn resolve(static_depth: u32, scope_level: u32) -> Self {
        use crate::arena::{FrameTarget, frame_target};
        match frame_target(static_depth as usize, scope_level as usize) {
            FrameTarget::Root => ScopeBinding::Root,
            FrameTarget::Top => ScopeBinding::Current,
            FrameTarget::Ancestor(_) => ScopeBinding::Ancestor,
        }
    }
}

/// Data for a pre-compiled exists check (boxed inside CompiledNode to reduce enum size).
#[cfg(feature = "ext-control")]
#[derive(Debug, Clone)]
pub(crate) struct CompiledExistsData {
    pub id: NodeId,
    pub scope_level: u32,
    pub segments: Box<[PathSegment]>,
    /// Compile-time frame resolution — see [`ScopeBinding`].
    pub binding: ScopeBinding,
}

/// Two-stage value: either resolved at compile time (`Now(S)`) or carried
/// as a [`CompiledNode`] (`Later(D)`) to be evaluated against the runtime
/// context. Used by every spot in `missing` / `missing_some` compilation
/// where an arg can be a literal we can pre-parse or an expression that
/// must wait until evaluation.
#[derive(Debug, Clone)]
pub(crate) enum Resolved<S, D> {
    /// Compile-time value — pre-parsed / pre-computed during compilation.
    Now(S),
    /// Runtime expression — evaluate against the live context.
    Later(D),
}

/// Pre-parsed `(raw_path, segments)` pair — the compile-time form of a
/// `missing` / `missing_some` path argument.
pub(crate) type StaticMissingPath = (Box<str>, Box<[PathSegment]>);

/// One arg to a `missing` / `missing_some` operator. Literal string paths
/// are pre-parsed into segments at compile time so the runtime walks the
/// input data without re-splitting the string or BTreeMap-keying via a
/// borrowed `&str` on every call.
pub(crate) type CompiledMissingArg = Resolved<StaticMissingPath, CompiledNode>;

/// `missing_some` minimum-present argument. `Now(usize)` is a literal
/// integer resolved at compile time; `Later(_)` is a runtime expression.
pub(crate) type CompiledMissingMin = Resolved<usize, CompiledNode>;

/// `missing_some` paths argument. `Now(_)` is a literal array of pre-parsed
/// paths; `Later(_)` is a runtime expression returning an array.
pub(crate) type CompiledMissingPaths = Resolved<Box<[StaticMissingPath]>, CompiledNode>;

/// Data for a pre-compiled `missing` operator.
#[derive(Debug, Clone)]
pub(crate) struct CompiledMissingData {
    pub id: NodeId,
    pub args: Box<[CompiledMissingArg]>,
}

/// Data for a pre-compiled `missing_some` operator. `min_present` may be a
/// literal integer (resolved at compile time) or a runtime expression.
#[derive(Debug, Clone)]
pub(crate) struct CompiledMissingSomeData {
    pub id: NodeId,
    pub min_present: CompiledMissingMin,
    pub paths: CompiledMissingPaths,
}

/// Data for a pre-compiled throw with a static error object.
/// Previously `Box<Value>`; upgraded to a named struct so it can carry an id
/// alongside the error payload.
#[cfg(feature = "error-handling")]
#[derive(Debug, Clone)]
pub(crate) struct CompiledThrowData {
    pub id: NodeId,
    pub error: OwnedDataValue,
}
