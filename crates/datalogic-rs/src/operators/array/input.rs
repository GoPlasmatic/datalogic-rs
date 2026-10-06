//! Iterator input: resolving an iterator's source into the shape the
//! operator bodies consume (and charging its items), and the per-item loops
//! that push a frame and dispatch the body.

use crate::arena::{ContextStack, DataValue, IterGuard};
use crate::node::{MetadataHint, ReduceHint};
use crate::{CompiledNode, Engine, Result};
use bumpalo::Bump;
use std::ops::ControlFlow;

// =============================================================================
// Iterator input resolution
// =============================================================================

/// Unified view over an iterator op's input collection. Single shape:
/// arena slice of `DataValue`. Wrapper kept for API stability.
#[derive(Clone, Copy)]
pub(crate) struct IterSrc<'a>(pub(crate) &'a [DataValue<'a>]);

impl<'a> IterSrc<'a> {
    #[inline]
    pub(crate) fn len(&self) -> usize {
        self.0.len()
    }

    #[inline]
    pub(crate) fn is_empty(&self) -> bool {
        self.0.is_empty()
    }

    /// Get item by index.
    #[inline]
    pub(crate) fn get(&self, i: usize) -> &'a DataValue<'a> {
        &self.0[i]
    }
}

/// Outcome of resolving an iterator op's first arg in arena mode.
pub(crate) enum ResolvedInput<'a> {
    /// Iterable input — proceed with array iteration.
    Iterable(IterSrc<'a>),
    /// Empty/null input — caller returns its empty-collection result.
    Empty,
    /// A non-array, non-null resolved value (Object, scalar, datetime, ...).
    /// Carries the resolved arena value so callers can dispatch natively
    /// (object-iteration / error / ...) without re-evaluating the arg.
    ///
    /// **Invariant:** `value_as_iter` routes arrays to [`Self::Iterable`] and
    /// null to [`Self::Empty`], so a `Bridge` payload is never
    /// `DataValue::Array` or `DataValue::Null`. Consumers rely on this to omit
    /// those match arms.
    Bridge(&'a DataValue<'a>),
}

/// An `each` row's source, resolved by the generated adapter: a null,
/// missing or empty-array source never reaches the body (the row's
/// `on_empty_source` answers it).
pub(crate) enum Items<'a> {
    /// A non-empty array.
    Array(IterSrc<'a>),
    /// An object, possibly empty: iterated as `(key, value)` pairs or
    /// rejected, per operator.
    Object(&'a [(&'a str, DataValue<'a>)]),
    /// Any other value (number, string, bool, datetime, ...).
    Scalar(&'a DataValue<'a>),
}

/// Compile-time classification of an iterator op's `args[0]` shape.
/// Stored on the parent `BuiltinOperator` (filter/map/all/some/none/reduce
/// /merge/min/max) and consulted by `resolve_iter_input` so the runtime
/// shape match collapses to a single byte compare.
///
/// `RootVarBorrow` covers the dominant pattern: `args[0]` is a plain
/// `{var: "..."}` against the root frame — we can read directly from
/// `ctx.root_input()` without dispatching into the arena evaluator. Any
/// other shape, including nested operators, falls through to `General`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum IterArgKind {
    /// `args[0]` is a `CompiledVar { scope_level: 0, … }` with no
    /// metadata/reduce/default — borrow directly from the root frame.
    /// `path_segments_empty == true` short-circuits the per-call segment
    /// length check inside `resolve_iter_input`.
    RootVarBorrow { path_segments_empty: bool },
    /// `args[0]` is anything else — evaluate via the dispatcher.
    General,
}

impl IterArgKind {
    /// Classify `args[0]` at compile time. Called from
    /// [`crate::node::populate_lits`] whenever the parent
    /// `BuiltinOperator` is one of the iterator ops listed above.
    pub(crate) fn classify(arg: &CompiledNode) -> Self {
        if let CompiledNode::Var {
            scope_level: 0,
            segments,
            reduce_hint: ReduceHint::None,
            metadata_hint: MetadataHint::None,
            default_value: None,
            ..
        } = arg
        {
            return IterArgKind::RootVarBorrow {
                path_segments_empty: segments.is_empty(),
            };
        }
        IterArgKind::General
    }
}

/// Resolve `args[0]` for an iterator op given the compile-time kind cached on
/// the parent. Two paths only:
///   - **Root borrow**: traverse `ctx.root_input()` directly when we can —
///     the dominant pattern in real workloads, reached in one byte compare.
///   - **General**: dispatch through the arena evaluator (covers composition
///     with another arena op, expressions, primitives — the dispatcher itself
///     handles those branches).
///
/// `ctx.depth() != 0` falls through to General even when the kind is
/// `RootVarBorrow`, because a borrow at non-root depth would leak the caller's
/// iteration frame instead of reading the rule's input.
///
/// `#[inline(always)]` because every iterator-op call funnels through here —
/// outlining was paying a function call for every quantifier/filter/map/
/// reduce despite the body being short and largely constant-foldable from
/// the call site (the cached `IterArgKind` tag).
#[inline(always)]
pub(crate) fn resolve_iter_input<'a>(
    arg: &'a CompiledNode,
    kind: IterArgKind,
    ctx: &mut ContextStack<'a>,
    engine: &Engine,
    arena: &'a Bump,
) -> Result<ResolvedInput<'a>> {
    if let IterArgKind::RootVarBorrow {
        path_segments_empty,
    } = kind
        && ctx.depth() == 0
    {
        let root = ctx.root_input();
        let av = if path_segments_empty {
            Some(root)
        } else if let CompiledNode::Var { segments, .. } = arg {
            crate::arena::value::traverse_segments(root, segments)
        } else {
            // Compile-time invariant violated; fall through to General path.
            None
        };
        if let Some(av) = av {
            return charged(value_as_iter(av), ctx);
        }
    }

    let av = engine.dispatch_node(arg, ctx, arena)?;
    charged(value_as_iter(av), ctx)
}

/// Charge one operation per item the caller is about to examine.
///
/// Every iterator operator funnels through [`resolve_iter_input`], so this
/// is the one place the per-item cost has to be taken — and taking it here,
/// before the caller chooses between its compile-time fast paths and the
/// general path, is what makes the cost independent of that choice. A fast
/// path that evaluates a predicate inline never dispatches the body and
/// would otherwise cost 0 per item, which would make whether a rule fits
/// its budget depend on which shape the populate pass recognised.
///
/// An object source arrives as `Bridge` and is charged one per pair, the
/// same as an array's one per item: a literal body costs nothing to
/// dispatch, so the body charge alone would price a `filter` over a large
/// object at a constant. Any other `Bridge` (a scalar) is charged 1.
#[inline(always)]
fn charged<'a>(input: ResolvedInput<'a>, ctx: &mut ContextStack<'a>) -> Result<ResolvedInput<'a>> {
    let items = match &input {
        ResolvedInput::Iterable(src) => src.len() as u64,
        ResolvedInput::Bridge(DataValue::Object(pairs)) => pairs.len() as u64,
        _ => 1,
    };
    ctx.charge(items)?;
    Ok(input)
}

/// Convert a resolved arena value into an `IterSrc` view, or signal Empty/Bridge.
#[inline]
fn value_as_iter<'a>(av: &'a DataValue<'a>) -> ResolvedInput<'a> {
    match av {
        DataValue::Null => ResolvedInput::Empty,
        DataValue::Array(items) => ResolvedInput::Iterable(IterSrc(items)),
        _ => ResolvedInput::Bridge(av),
    }
}

// =============================================================================
// Per-iteration body machinery (filter / map / quantifiers)
// =============================================================================
//
// `for_each_iter_array` / `for_each_iter_object` factor out the common loop:
// `IterGuard::new` + `step_indexed`/`step_keyed` + `run_iter_body`. Each call
// site supplies a closure that consumes `(i, item, [key], body_result)` and
// returns `ControlFlow` to support short-circuit (quantifiers); filter/map
// always return `Continue`.
//
// Reduce is intentionally NOT factored through these — its `step_reduce`
// frame shape (item + accumulator) differs from the indexed/keyed shape.

/// Iterate `items` with an indexed iter frame pushed for each element.
/// `step_fn` receives `(i, item, body_result)` and may break the loop.
#[inline]
pub(super) fn for_each_iter_array<'a, F>(
    items: &'a [DataValue<'a>],
    body: &'a CompiledNode,
    ctx: &mut ContextStack<'a>,
    engine: &Engine,
    arena: &'a Bump,
    mut step_fn: F,
) -> Result<()>
where
    F: FnMut(usize, &'a DataValue<'a>, &'a DataValue<'a>) -> Result<ControlFlow<()>>,
{
    let total = items.len() as u32;
    let mut guard = IterGuard::new(ctx);
    for (i, item_av) in items.iter().enumerate() {
        guard.step_indexed(item_av, i);
        let av = engine.run_iter_body(body, guard.stack(), arena, i as u32, total)?;
        if step_fn(i, item_av, av)?.is_break() {
            break;
        }
    }
    drop(guard);
    Ok(())
}

/// Iterate object `pairs` with a keyed iter frame pushed for each (key, value).
/// `step_fn` receives `(i, item, key, body_result)` and may break the loop.
#[inline]
pub(super) fn for_each_iter_object<'a, F>(
    pairs: &'a [(&'a str, DataValue<'a>)],
    body: &'a CompiledNode,
    ctx: &mut ContextStack<'a>,
    engine: &Engine,
    arena: &'a Bump,
    mut step_fn: F,
) -> Result<()>
where
    F: FnMut(usize, &'a DataValue<'a>, &'a str, &'a DataValue<'a>) -> Result<ControlFlow<()>>,
{
    let total = pairs.len() as u32;
    let mut guard = IterGuard::new(ctx);
    for (i, (k, v)) in pairs.iter().enumerate() {
        guard.step_keyed(v, i, k);
        let av = engine.run_iter_body(body, guard.stack(), arena, i as u32, total)?;
        if step_fn(i, v, k, av)?.is_break() {
            break;
        }
    }
    drop(guard);
    Ok(())
}
