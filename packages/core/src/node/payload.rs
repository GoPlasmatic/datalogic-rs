//! Boxed payload types and small helper enums referenced by
//! [`super::CompiledNode`]. Split out so the enum file stays focused on the
//! variant list and dispatch helpers.

use super::CompiledNode;
use super::compile_ctx::NodeId;
use datavalue::OwnedDataValue;

/// A pre-parsed path segment for compiled variable access.
#[derive(Debug, Clone)]
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
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
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
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
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

/// Data for a structured object template (boxed inside CompiledNode to reduce enum size).
#[cfg(feature = "templating")]
#[derive(Debug, Clone)]
pub(crate) struct StructuredObjectData {
    pub id: NodeId,
    pub fields: Box<[(String, CompiledNode)]>,
}

/// Data for a pre-compiled exists check (boxed inside CompiledNode to reduce enum size).
#[cfg(feature = "ext-control")]
#[derive(Debug, Clone)]
pub(crate) struct CompiledExistsData {
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
