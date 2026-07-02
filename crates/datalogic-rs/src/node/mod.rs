//! Compiled node tree shared between the compile pipeline and the dispatch
//! engine. Submodules are structured by concern:
//!
//! - [`compile_ctx`] — `NodeId` encoding, the synthetic-id sentinel,
//!   and the per-compile id counter.
//! - [`payload`] — boxed payload structs and small helper enums
//!   referenced by `CompiledNode` variants.
//! - [`populate`] — post-compile pass that caches per-operator analysis
//!   results onto every `BuiltinOperator` node.
//! - [`logic`] — `Logic`, the public compiled-rule snapshot, and the
//!   static-evaluation predicates the optimizer consults.
//!
//! Re-exports below preserve the pre-split `crate::node::*` import paths so
//! callers elsewhere in the crate are unaffected by the file split.

mod compile_ctx;
mod logic;
mod payload;
mod populate;

pub use logic::Logic;

pub(crate) use compile_ctx::{CompileCtx, NodeId, SYNTHETIC_ID};
pub(crate) use logic::node_is_static;
#[cfg(feature = "ext-control")]
pub(crate) use payload::CompiledExistsData;
#[cfg(feature = "error-handling")]
pub(crate) use payload::CompiledThrowData;
#[cfg(feature = "templating")]
pub(crate) use payload::StructuredObjectData;
pub(crate) use payload::{
    CompiledMissingArg, CompiledMissingData, CompiledMissingMin, CompiledMissingPaths,
    CompiledMissingSomeData, CustomOperatorData, MetadataHint, PathSegment, ReduceHint,
};
pub(crate) use populate::populate_lits;

use crate::arena::DataValue;
use crate::opcode::OpCode;
use datavalue::OwnedDataValue;
use populate::precompute_lit;
use std::num::NonZeroU32;

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
pub(crate) enum CompiledNode {
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
    /// `predicate_hint` caches the result of [`crate::operators::array::FastPredicate::try_detect_owned`]
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

    /// A structured object template for templating mode.
    /// Boxed to reduce enum size (rare variant).
    #[cfg(feature = "templating")]
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

    /// Compile-time placeholder for an operator invoked with malformed
    /// args (e.g. `and` / `or` / `if` with a non-array argument). The
    /// dispatcher raises an `InvalidArguments` error on encounter — this
    /// variant exists so the diagnostic surfaces at runtime via the
    /// normal error breadcrumb path rather than at compile time. The
    /// `op_name` is captured from the source-text op so the runtime error
    /// names *which* op was misused (e.g. "Invalid arguments: if") even
    /// when the failure is nested inside an outer op.
    InvalidArgs { id: NodeId, op_name: &'static str },
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
    pub(crate) fn id(&self) -> u32 {
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
            #[cfg(feature = "templating")]
            CompiledNode::StructuredObject(data) => data.id,
            CompiledNode::Var { id, .. } => *id,
            #[cfg(feature = "ext-control")]
            CompiledNode::Exists(data) => data.id,
            #[cfg(feature = "error-handling")]
            CompiledNode::Throw(data) => data.id,
            CompiledNode::Missing(data) => data.id,
            CompiledNode::MissingSome(data) => data.id,
            CompiledNode::InvalidArgs { id, .. } => *id,
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
            #[cfg(feature = "templating")]
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
            CompiledNode::InvalidArgs { .. } => {}
        }
    }

    /// Mutable mirror of [`Self::visit_indexed_children`]. Used by the post-compile
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
            #[cfg(feature = "templating")]
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
            CompiledNode::InvalidArgs { .. } => {}
        }
    }

    /// Convenience constructor for a `Value` node with a [`SYNTHETIC_ID`].
    ///
    /// Used by operator fast paths that wrap runtime values back into
    /// `CompiledNode::Value` purely for dispatch. These wrappers are never
    /// observed by tracing or error reporting, so assigning a real id would
    /// be misleading.
    #[inline]
    pub(crate) fn synthetic_value(value: OwnedDataValue) -> Self {
        Self::value_with_id(SYNTHETIC_ID, value)
    }

    /// Construct a `CompiledNode::Value` with `id` and `value`, populating
    /// the precomputed `lit` for primitive literals so the arena
    /// dispatch hot path can borrow it without a per-call `arena.alloc`.
    /// Centralised here so every construction site stays in sync — adding
    /// a new precomputable variant only requires editing
    /// [`populate::precompute_lit`].
    #[inline]
    pub(crate) fn value_with_id(id: NodeId, value: OwnedDataValue) -> Self {
        let lit = precompute_lit(&value);
        CompiledNode::Value { id, value, lit }
    }

    /// Returns the name of this node's top-level operator, if any.
    ///
    /// Used when wrapping an error with structured context — we only report
    /// the outermost operator, not the full nested call chain. Borrows a
    /// static name for built-ins and the named compiled forms; allocates
    /// only for `CustomOperator` (the user-supplied name). `Logic` caches the
    /// root node's result at compile time (see `Logic::new`).
    pub(crate) fn operator_name(&self) -> Option<std::borrow::Cow<'static, str>> {
        use std::borrow::Cow;
        match self {
            CompiledNode::BuiltinOperator { opcode, .. } => Some(Cow::Borrowed(opcode.as_str())),
            CompiledNode::CustomOperator(data) => Some(Cow::Owned(data.name.clone())),
            CompiledNode::Var { .. } => Some(Cow::Borrowed("var")),
            #[cfg(feature = "ext-control")]
            CompiledNode::Exists(_) => Some(Cow::Borrowed("exists")),
            #[cfg(feature = "error-handling")]
            CompiledNode::Throw(_) => Some(Cow::Borrowed("throw")),
            CompiledNode::Missing(_) => Some(Cow::Borrowed("missing")),
            CompiledNode::MissingSome(_) => Some(Cow::Borrowed("missing_some")),
            CompiledNode::InvalidArgs { op_name, .. } => Some(Cow::Borrowed(op_name)),
            _ => None,
        }
    }
}

#[cfg(test)]
mod layout_tests {
    /// `CompiledNode` is the hot dispatch type — every evaluation walks
    /// slices of it, so its size directly drives cache behaviour. 48
    /// bytes is the current all-features size on 64-bit; a variant that
    /// pushes it past that belongs behind a `Box`.
    #[test]
    #[cfg(target_pointer_width = "64")]
    fn compiled_node_stays_small() {
        assert!(
            std::mem::size_of::<super::CompiledNode>() <= 48,
            "CompiledNode grew past 48 bytes ({}); box the payload of the offending variant",
            std::mem::size_of::<super::CompiledNode>()
        );
    }
}
