//! Arena-mode dispatch hub.
//!
//! [`dispatch_node_inner`] is the exhaustive `CompiledNode` match that
//! routes each node shape to its implementation; built-in operators are
//! routed by the operator table's generated `dispatch_builtin`. It is invoked from
//! `Engine::dispatch_node`, which handles the literal fast path,
//! breadcrumb accumulation, and trace recording before delegating here.

use super::Engine;
use crate::arena::ContextStack;
use crate::operators::table::dispatch_builtin;
use crate::{CompiledNode, Error, Result};

/// Inner dispatch — never called directly; reachable only via
/// `Engine::dispatch_node` which handles the literal fast path,
/// breadcrumb accumulation, and trace recording.
///
/// `#[inline(always)]` is load-bearing here: this function is the hot
/// dispatch and the compiler inlines it into `dispatch_node` in the
/// single-file layout. Crossing the module boundary loses that inline
/// decision (measured ~1 ns regression on the 15 ns baseline).
///
/// Built-in operators go through [`dispatch_builtin`], which the operator
/// table generates (`#[inline(always)]` too, so its `match` lowers into
/// the same jump table). The arms written here are node kinds, not
/// operators: compiled forms (`Var`, `Exists`, `Missing`, `Throw`),
/// literals, templates and the CSE wrapper.
///
/// The heavy `bumpalo::Vec`-building cases (`Array`, `StructuredObject`,
/// `CustomOperator`) live in `#[inline(never)]` helpers below, so the
/// dispatch's stack frame is sized for the small/common arms regardless
/// of which arm fires.
#[inline(always)]
pub(super) fn dispatch_node_inner<'a>(
    engine: &Engine,
    node: &'a CompiledNode,
    ctx: &mut ContextStack<'a>,
    arena: &'a bumpalo::Bump,
) -> Result<&'a crate::arena::DataValue<'a>> {
    match node {
        CompiledNode::BuiltinOperator {
            opcode,
            args,
            iter_arg_kind,
            ..
        } => dispatch_builtin(*opcode, args, *iter_arg_kind, ctx, engine, arena),

        // Compiled var: full dispatch via the arena helper. Root and
        // frame data are both arena-resident `DataValue`s, so lookups
        // are zero-copy borrows.
        CompiledNode::Var {
            scope_level,
            segments,
            reduce_hint,
            metadata_hint,
            default_value,
            binding,
            ..
        } => crate::operators::variable::evaluate_val_compiled(
            crate::operators::variable::CompiledVarSpec {
                scope_level: *scope_level,
                segments,
                reduce_hint: *reduce_hint,
                metadata_hint: *metadata_hint,
                default_value: default_value.as_deref(),
                binding: *binding,
            },
            ctx,
            engine,
            arena,
        ),

        // Compiled exists: full dispatch — root scope walks the input
        // directly, others walk arena frame data. Result is always a
        // Bool singleton.
        #[cfg(feature = "ext-control")]
        CompiledNode::Exists(data) => crate::operators::variable::evaluate_exists_compiled(
            data.scope_level,
            &data.segments,
            data.binding,
            ctx,
        ),

        // Value literal: the outer `dispatch_node` wrapper already
        // routes `Value` through `literal_fallback`, so this arm is
        // unreachable in normal flow. Defensive fallback — degrades
        // gracefully to a fresh arena alloc if a future code path
        // ever calls the inner dispatcher directly with a literal.
        CompiledNode::Value { value, .. } => Ok(super::literal_fallback(value, arena)),

        // Compiled missing / missing_some — pre-parsed segments.
        CompiledNode::Missing(data) => {
            crate::operators::missing::evaluate_compiled_missing(data, ctx, engine, arena)
        }
        CompiledNode::MissingSome(data) => {
            crate::operators::missing::evaluate_compiled_missing_some(data, ctx, engine, arena)
        }

        // Compile-time placeholder for a call known to be malformed
        // (`and` / `or` / `if` with a non-array argument, a literal
        // unknown timezone). Keep the canonical "Invalid Arguments"
        // message (the JSONLogic suite uses it as the error tag) but
        // attach the captured op name to the operator field so nested
        // failures still identify the misused op.
        CompiledNode::InvalidArgs { op_name, .. } => {
            Err(crate::Error::invalid_args().with_operator(*op_name))
        }

        // CompiledThrow — constant-folded error literal. Inside a
        // protected `try` arm (and untraced) the error can't escape:
        // park the payload arena-borrowed in the context's thrown slot
        // and skip the owned deep clone — `try`'s catch arm reads the
        // slot back without an owned→arena round-trip. `data.error` is
        // already normalized (`{"type": ...}`) by the compile-time fold.
        #[cfg(feature = "error-handling")]
        CompiledNode::Throw(data) => {
            if ctx.in_catch_scope() && !ctx.is_tracing() {
                let av: &crate::arena::DataValue = arena.alloc(data.error.view_in(arena));
                ctx.set_thrown_slot(av);
                Err(Error::deferred_thrown())
            } else {
                Err(Error::thrown(data.error.clone()))
            }
        }

        // Out-of-line — bumpalo::Vec construction would otherwise force
        // a large stack frame on every dispatch arm via worst-case
        // spill sizing. See the comments on the helpers below.
        #[cfg(feature = "templating")]
        CompiledNode::StructuredObject(data) => {
            evaluate_structured_object(data, ctx, engine, arena)
        }
        CompiledNode::Array { nodes, .. } => evaluate_array_literal(nodes, ctx, engine, arena),
        CompiledNode::CustomOperator(data) => evaluate_custom_operator(data, ctx, engine, arena),

        // CSE memo wrapper — one more jump-table entry, so non-CSE
        // nodes pay nothing for the feature. See `dispatch_cse`.
        CompiledNode::Cse(data) => dispatch_cse(engine, data, ctx, arena),
    }
}

// Heavy arms below are kept out-of-line so the dispatch fn's stack frame
// is sized for the small/common arms only. Each builds a `bumpalo::Vec`
// (multi-word locals + drop glue) which, when inlined, forced the
// dispatch prologue to reserve ~464 B of stack on every recursive call.
// `#[inline(never)]` is load-bearing — see the comment on
// `dispatch_node_inner`.

/// Evaluate a CSE memo wrapper: consult/fill the per-evaluation slot when
/// the double runtime gate holds. At `depth() == 0` a pure subtree is a
/// function of (root data, engine config) only — up-level `val`s clamp to
/// root — so the memoized borrow is context-safe; under an iterator/catch
/// frame or an attached tracer the memo is bypassed and the wrapper
/// degrades to a plain delegating dispatch. Only `Ok` results are cached:
/// errors re-evaluate deterministically (e.g. after a `try` caught the
/// first occurrence's failure).
///
/// Delegates to [`dispatch_node_inner`] rather than the outer
/// `dispatch_node`, so the caller's wrapper performs the breadcrumb push
/// and trace recording exactly once, attributed to the wrapped node's
/// delegated id — error paths and traces stay byte-identical to an
/// unwrapped tree.
#[inline(never)]
fn dispatch_cse<'a>(
    engine: &Engine,
    data: &'a crate::node::CseData,
    ctx: &mut ContextStack<'a>,
    arena: &'a bumpalo::Bump,
) -> Result<&'a crate::arena::DataValue<'a>> {
    // The depth half of the old gate is a compile-time tautology: the CSE pass
    // never descends into a child position for which
    // `compile::scope::frames_pushed_for_child` is non-zero, so every `Cse`
    // node in a compiled tree sits at static frame depth 0. Only the tracer
    // half is genuinely dynamic. The assertion below pins the invariant from
    // the runtime side; `cse::tests::no_cse_node_sits_under_a_pushed_frame`
    // pins it from the compile side.
    debug_assert_eq!(
        ctx.depth(),
        0,
        "Cse node dispatched under a pushed frame — the memo would leak across frames"
    );
    if !ctx.is_tracing() {
        if let Some(hit) = ctx.cse_slot(data.slot) {
            return Ok(hit);
        }
        let result = dispatch_node_inner(engine, &data.inner, ctx, arena);
        if let Ok(value) = &result {
            ctx.fill_cse_slot(data.slot, value);
        }
        result
    } else {
        dispatch_node_inner(engine, &data.inner, ctx, arena)
    }
}

#[cfg(feature = "templating")]
#[inline(never)]
fn evaluate_structured_object<'a>(
    data: &'a crate::node::StructuredObjectData,
    ctx: &mut crate::arena::ContextStack<'a>,
    engine: &super::Engine,
    arena: &'a bumpalo::Bump,
) -> crate::Result<&'a crate::arena::DataValue<'a>> {
    use crate::arena::DataValue;
    if data.fields.is_empty() {
        return Ok(crate::arena::singletons::singleton_empty_object());
    }
    // Keys are stored in their source form, so an escaped template strips
    // one prefix per key here. Hoisted out of the loop and gated on the
    // compile-time flag: an ordinary template never even looks at the
    // engine setting. `strip_prefix` takes a `char`, so a multi-byte
    // escape is handled without any byte-boundary arithmetic.
    let escape = if data.has_escaped_keys {
        engine.template_key_escape()
    } else {
        None
    };
    let mut pairs: bumpalo::collections::Vec<'a, (&'a str, DataValue<'a>)> =
        bumpalo::collections::Vec::with_capacity_in(data.fields.len(), arena);
    for (key, n) in data.fields.iter() {
        let val_av = engine.dispatch_node(n, ctx, arena)?;
        let val_owned = *val_av;
        let key = match escape {
            Some(c) => key.strip_prefix(c).unwrap_or(key),
            None => key.as_str(),
        };
        let k: &'a str = arena.alloc_str(key);
        pairs.push((k, val_owned));
    }
    Ok(arena.alloc(DataValue::Object(pairs.into_bump_slice())))
}

#[inline(never)]
fn evaluate_array_literal<'a>(
    nodes: &'a [crate::CompiledNode],
    ctx: &mut crate::arena::ContextStack<'a>,
    engine: &super::Engine,
    arena: &'a bumpalo::Bump,
) -> crate::Result<&'a crate::arena::DataValue<'a>> {
    use crate::arena::DataValue;
    if nodes.is_empty() {
        return Ok(crate::arena::singletons::singleton_empty_array());
    }
    let mut items: bumpalo::collections::Vec<'a, DataValue<'a>> =
        bumpalo::collections::Vec::with_capacity_in(nodes.len(), arena);
    for n in nodes.iter() {
        let av = engine.dispatch_node(n, ctx, arena)?;
        items.push(*av);
    }
    Ok(arena.alloc(DataValue::Array(items.into_bump_slice())))
}

#[inline(never)]
fn evaluate_custom_operator<'a>(
    data: &'a crate::node::CustomOperatorData,
    ctx: &mut crate::arena::ContextStack<'a>,
    engine: &super::Engine,
    arena: &'a bumpalo::Bump,
) -> crate::Result<&'a crate::arena::DataValue<'a>> {
    use crate::arena::DataValue;
    let op = engine
        .custom_operators
        .get(&data.name)
        .ok_or_else(|| Error::invalid_operator(data.name.clone()))?;
    let mut args: bumpalo::collections::Vec<'a, &'a DataValue<'a>> =
        bumpalo::collections::Vec::with_capacity_in(data.args.len(), arena);
    for arg in data.args.iter() {
        args.push(engine.dispatch_node(arg, ctx, arena)?);
    }
    let mut wrapped = crate::operator::EvalContext::new(ctx);
    op.evaluate(&args, &mut wrapped, arena)
}
