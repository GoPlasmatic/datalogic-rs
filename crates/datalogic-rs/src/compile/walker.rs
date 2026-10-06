//! `compile_node` and friends: convert an [`OwnedDataValue`] rule tree into
//! the engine's [`CompiledNode`] form.
//!
//! `compile_node` dispatches by [`OwnedDataValue`] variant. The interesting
//! case is a single-key object — that's an operator invocation, and
//! operator-specific specialisations live in `super::operator`.

use datavalue::OwnedDataValue;

use crate::OpCode;
use crate::node::{CompileCtx, CompiledNode, node_is_static};
use crate::{Engine, Result};

use super::optimize;
use crate::operators::meta::{ArgsForm, CompileHook, HookArgs, Hooked, LiteralArgs};

/// Compile a single value into a [`CompiledNode`].
///
/// Wraps the recursive descent in a depth guard: each nesting level bumps the
/// compile-time depth counter and bails with a `ConfigurationError` once it
/// passes `MAX_COMPILE_DEPTH`. This bounds a programmatically-built rule
/// (which reaches the compiler via `IntoLogic` without the JSON parser's own
/// depth cap) so it can't overflow the stack here, in dispatch, or in the
/// recursive `Drop` of the compiled tree.
pub(super) fn compile_node(
    value: &OwnedDataValue,
    engine: Option<&Engine>,
    templating: bool,
    ctx: &mut CompileCtx,
) -> Result<CompiledNode> {
    ctx.enter()?;
    let result = compile_node_inner(value, engine, templating, ctx);
    ctx.leave();
    result
}

fn compile_node_inner(
    value: &OwnedDataValue,
    engine: Option<&Engine>,
    templating: bool,
    ctx: &mut CompileCtx,
) -> Result<CompiledNode> {
    match value {
        OwnedDataValue::Object(pairs) if pairs.len() > 1 => {
            compile_multi_key_object(pairs, engine, templating, ctx)
        }
        OwnedDataValue::Object(pairs) if pairs.len() == 1 => {
            let (op_name, args_value) = &pairs[0];
            compile_operator_invocation(op_name, args_value, engine, templating, true, ctx)
        }
        OwnedDataValue::Array(arr) => compile_array(arr, engine, templating, ctx),
        _ => Ok(CompiledNode::value_with_id(
            Some(ctx.next_id()),
            value.clone(),
        )),
    }
}

/// Multi-key object — only valid in `templating` mode (where it
/// becomes a structured-object output template); otherwise an error.
fn compile_multi_key_object(
    pairs: &[(String, OwnedDataValue)],
    engine: Option<&Engine>,
    templating: bool,
    ctx: &mut CompileCtx,
) -> Result<CompiledNode> {
    #[cfg(feature = "templating")]
    if templating {
        let fields: Vec<_> = pairs
            .iter()
            .map(|(key, val)| {
                keyed(key, ctx, |ctx| compile_node(val, engine, templating, ctx))
                    .map(|compiled_val| (key.clone(), compiled_val))
            })
            .collect::<Result<Vec<_>>>()?;
        // Multi-key object keys are already literal, so the escape changes
        // nothing about *routing* here — it only has to be recorded so the
        // evaluator strips the prefix and folding leaves the node alone.
        let escape = engine.and_then(|e| e.template_key_escape());
        let has_escaped_keys = key_escape_present(&fields, escape);
        return Ok(CompiledNode::StructuredObject(Box::new(
            crate::node::StructuredObjectData {
                id: Some(ctx.next_id()),
                fields: fields.into_boxed_slice(),
                has_escaped_keys,
            },
        )));
    }
    let _ = (pairs, engine, templating, ctx);
    Err(crate::error::Error::invalid_operator("Unknown Operator"))
}

/// Single-key object: an operator invocation. Routes to either the builtin
/// path (when the key parses as an `OpCode`) or the custom-operator /
/// templating-mode path.
///
/// `fold` is false for an argument the parent reads as written (see
/// [`compile_as_written`]): the call is then neither optimized nor folded
/// itself, though its own arguments still are.
fn compile_operator_invocation(
    op_name: &str,
    args_value: &OwnedDataValue,
    engine: Option<&Engine>,
    templating: bool,
    fold: bool,
    ctx: &mut CompileCtx,
) -> Result<CompiledNode> {
    // The escape check runs *before* operator resolution — that ordering is
    // the whole feature. It's what lets an escaped key name a built-in
    // (`$type`) or a registered custom operator (`$my_op`) and still come
    // out as a literal output field.
    #[cfg(feature = "templating")]
    if templating {
        // No let-chain here: the crate's MSRV is 1.85 and they only
        // stabilised in 1.88.
        let escape = engine.and_then(|e| e.template_key_escape());
        if escape.is_some_and(|c| op_name.starts_with(c)) {
            return single_field_object(op_name, args_value, engine, templating, true, ctx);
        }
    }

    let builtin = match engine {
        Some(engine) => engine.builtin(op_name),
        None => op_name.parse::<OpCode>().ok(),
    };
    if let Some(opcode) = builtin {
        return compile_builtin(op_name, opcode, args_value, engine, templating, fold, ctx);
    }

    #[cfg(feature = "templating")]
    if templating {
        return compile_templating_unknown(op_name, args_value, engine, templating, fold, ctx);
    }

    let args = keyed(op_name, ctx, |ctx| {
        compile_args(args_value, engine, templating, ctx)
    })?;
    Ok(custom_operator_node(op_name, args, engine, fold, ctx))
}

/// Build a `CustomOperator` node from an op name and its already-compiled
/// args, recording what the operator declares about itself. A call to an
/// operator that declares itself deterministic and context-free, with
/// constant arguments, is evaluated now and replaced by its result (unless
/// the parent reads this argument as written).
fn custom_operator_node(
    op_name: &str,
    args: Box<[CompiledNode]>,
    engine: Option<&Engine>,
    fold: bool,
    ctx: &mut CompileCtx,
) -> CompiledNode {
    let info = engine
        .and_then(|e| e.custom_operator_info(op_name))
        .unwrap_or_else(crate::CustomOperatorInfo::opaque);
    let node = CompiledNode::CustomOperator(Box::new(crate::node::CustomOperatorData {
        id: Some(ctx.next_id()),
        name: op_name.to_string(),
        args,
        info,
        slot: engine.and_then(|e| e.custom_operator_slot(op_name)),
    }));
    if let Some(eng) = engine
        && fold
        && !ctx.skip_fold()
        && node_is_static(&node)
        && let Some(value) = optimize::constant_fold::fold_static_node(&node, eng)
    {
        return CompiledNode::compile_time_value(Some(ctx.next_id()), value);
    }
    node
}

/// Builtin operator path: the row's argument-form rule, its compile hook
/// (if any), then a generic `BuiltinOperator` with the optimization and
/// static-fold passes when an `engine` is supplied.
fn compile_builtin(
    op_name: &str,
    opcode: OpCode,
    args_value: &OwnedDataValue,
    engine: Option<&Engine>,
    templating: bool,
    fold: bool,
    ctx: &mut CompileCtx,
) -> Result<CompiledNode> {
    let meta = opcode.meta();
    if meta.args_form == ArgsForm::ArrayOnly && !matches!(args_value, OwnedDataValue::Array(_)) {
        return Ok(invalid_args_marker(opcode, args_value, ctx));
    }

    if let Some(CompileHook::Raw(hook)) = meta.compile
        && let Some(node) = hook(args_value, ctx)
    {
        return Ok(node);
    }

    let mut args = keyed(op_name, ctx, |ctx| {
        compile_builtin_args(meta.literal_args, args_value, engine, templating, ctx)
    })?;

    if let Some(CompileHook::Args(hook)) = meta.compile {
        let hook_args = HookArgs {
            opcode,
            op_name,
            args_value,
            ctx,
        };
        match hook(args, hook_args) {
            Hooked::Node(node) => return Ok(node),
            Hooked::Generic(back) => args = back,
        }
    }

    let mut node = CompiledNode::BuiltinOperator {
        id: Some(ctx.next_id()),
        opcode,
        args,
        predicate_hint: None,
        iter_arg_kind: crate::operators::array::IterArgKind::General,
    };

    // Optimization + static-fold passes (engine-dependent and gated on
    // the compile context's `skip_fold` flag, which the trace path sets).
    // Folded literals are built with `compile_time_value` so composite
    // results carry their prebuilt view immediately — an enclosing static
    // operator folded right after this consumes it structurally (e.g.
    // `evaluate_switch`'s folded-case-table arms).
    if let Some(eng) = engine
        && fold
        && !ctx.skip_fold()
    {
        node = optimize::optimize(node, eng);
        if node_is_static(&node)
            && let Some(value) = optimize::constant_fold::fold_static_node(&node, eng)
        {
            return Ok(CompiledNode::compile_time_value(Some(ctx.next_id()), value));
        }
    }

    Ok(node)
}

/// Build the [`CompiledNode::InvalidArgs`] placeholder for a malformed
/// call that is known to fail at compile time (`and` / `or` / `if` with a
/// non-array argument, a literal unknown timezone). Carries the op name
/// forward so the dispatcher can produce an error that names the failing
/// op rather than a generic "Invalid Arguments", and the raw `args_value`
/// so `to_json` can reproduce the offending rule verbatim instead of a
/// placeholder that re-parses as something else.
pub(super) fn invalid_args_marker(
    opcode: OpCode,
    args_value: &OwnedDataValue,
    ctx: &mut CompileCtx,
) -> CompiledNode {
    CompiledNode::InvalidArgs {
        id: Some(ctx.next_id()),
        op_name: opcode.as_str(),
        args: Box::new(args_value.clone()),
    }
}

/// Unknown-operator handling under `templating` mode. Custom
/// operators registered on the engine compile to a `CustomOperator`;
/// otherwise the key/value pair becomes a single-field structured-object
/// output template.
#[cfg(feature = "templating")]
fn compile_templating_unknown(
    op_name: &str,
    args_value: &OwnedDataValue,
    engine: Option<&Engine>,
    templating: bool,
    fold: bool,
    ctx: &mut CompileCtx,
) -> Result<CompiledNode> {
    if let Some(eng) = engine
        && eng.has_custom_operator(op_name)
    {
        let args = keyed(op_name, ctx, |ctx| {
            compile_args(args_value, engine, templating, ctx)
        })?;
        return Ok(custom_operator_node(op_name, args, engine, fold, ctx));
    }
    single_field_object(op_name, args_value, engine, templating, false, ctx)
}

/// Compile `{key: value}` into a one-field structured-object template.
///
/// Shared by the two templating routes that produce one: an unknown
/// operator key (which is just a literal field), and an escaped key (which
/// bypassed operator resolution entirely). `escaped` records which route
/// arrived here — see [`crate::node::StructuredObjectData::has_escaped_keys`].
#[cfg(feature = "templating")]
fn single_field_object(
    key: &str,
    value: &OwnedDataValue,
    engine: Option<&Engine>,
    templating: bool,
    escaped: bool,
    ctx: &mut CompileCtx,
) -> Result<CompiledNode> {
    let compiled_val = keyed(key, ctx, |ctx| compile_node(value, engine, templating, ctx))?;
    let fields = vec![(key.to_string(), compiled_val)].into_boxed_slice();
    Ok(CompiledNode::StructuredObject(Box::new(
        crate::node::StructuredObjectData {
            id: Some(ctx.next_id()),
            fields,
            has_escaped_keys: escaped,
        },
    )))
}

/// Whether any field key carries `escape`. `None` (no escape configured)
/// short-circuits to `false` so unescaped templates skip the scan.
#[cfg(feature = "templating")]
fn key_escape_present(fields: &[(String, CompiledNode)], escape: Option<char>) -> bool {
    let Some(escape) = escape else {
        return false;
    };
    fields.iter().any(|(key, _)| key.starts_with(escape))
}

/// Compile a literal array. When all elements are static and an engine is
/// supplied, the whole array is constant-folded to an [`OwnedDataValue`] literal.
fn compile_array(
    arr: &[OwnedDataValue],
    engine: Option<&Engine>,
    templating: bool,
    ctx: &mut CompileCtx,
) -> Result<CompiledNode> {
    let nodes = arr
        .iter()
        .enumerate()
        .map(|(i, v)| indexed(i, ctx, |ctx| compile_node(v, engine, templating, ctx)))
        .collect::<Result<Vec<_>>>()?;

    let nodes_boxed = nodes.into_boxed_slice();
    let node = CompiledNode::Array {
        id: Some(ctx.next_id()),
        nodes: nodes_boxed,
    };

    if let Some(eng) = engine
        && !ctx.skip_fold()
        && node_is_static(&node)
        && let Some(value) = optimize::constant_fold::fold_static_node(&node, eng)
    {
        // `compile_time_value`: the folded array carries its
        // prebuilt composite view immediately, so an enclosing
        // static operator folded during this same compile (e.g. a
        // literal-discriminant `switch` matching its case table
        // via `lit: Some`) evaluates correctly at fold time.
        return Ok(CompiledNode::compile_time_value(Some(ctx.next_id()), value));
    }

    Ok(node)
}

/// Compile a built-in's arguments. A position whose literal shape the
/// operator reads ([`LiteralArgs`]) is compiled [as written](compile_as_written).
fn compile_builtin_args(
    literal: LiteralArgs,
    value: &OwnedDataValue,
    engine: Option<&Engine>,
    templating: bool,
    ctx: &mut CompileCtx,
) -> Result<Box<[CompiledNode]>> {
    if literal == LiteralArgs::None {
        return compile_args(value, engine, templating, ctx);
    }
    let (items, listed) = match value {
        OwnedDataValue::Array(arr) => (arr.as_slice(), true),
        other => (std::slice::from_ref(other), false),
    };
    let len = items.len();
    items
        .iter()
        .enumerate()
        .map(|(i, v)| {
            let compile = |ctx: &mut CompileCtx| {
                if literal.reads(i, len) {
                    compile_as_written(v, engine, templating, ctx)
                } else {
                    compile_node(v, engine, templating, ctx)
                }
            };
            // A lone argument is the operator's value itself, not item 0.
            if listed {
                indexed(i, ctx, compile)
            } else {
                compile(ctx)
            }
        })
        .collect::<Result<Vec<_>>>()
        .map(Vec::into_boxed_slice)
}

/// Whether `value` holds an operator call or template anywhere outside a
/// nested call: what makes an argument an expression rather than a
/// literal.
fn has_expression(value: &OwnedDataValue) -> bool {
    match value {
        OwnedDataValue::Object(pairs) => !pairs.is_empty(),
        OwnedDataValue::Array(items) => items.iter().any(has_expression),
        _ => false,
    }
}

/// Compile an argument so that its node says whether it was written as a
/// literal. A call keeps its own node instead of folding into a `Value`,
/// and an array holding an expression stays an `Array` node, so an
/// operator that reads literal shape sees a literal only where the rule
/// has one. Everything below the root still optimizes and folds.
fn compile_as_written(
    value: &OwnedDataValue,
    engine: Option<&Engine>,
    templating: bool,
    ctx: &mut CompileCtx,
) -> Result<CompiledNode> {
    if !has_expression(value) {
        return compile_node(value, engine, templating, ctx);
    }
    ctx.enter()?;
    let result = match value {
        OwnedDataValue::Object(pairs) if pairs.len() == 1 => {
            let (op_name, args_value) = &pairs[0];
            compile_operator_invocation(op_name, args_value, engine, templating, false, ctx)
        }
        OwnedDataValue::Array(items) => items
            .iter()
            .enumerate()
            .map(|(i, v)| indexed(i, ctx, |ctx| compile_as_written(v, engine, templating, ctx)))
            .collect::<Result<Vec<_>>>()
            .map(|nodes| CompiledNode::Array {
                id: Some(ctx.next_id()),
                nodes: nodes.into_boxed_slice(),
            }),
        // A template is never folded into a literal.
        other => compile_node_inner(other, engine, templating, ctx),
    };
    ctx.leave();
    result
}

/// Compile operator arguments — an array is iterated; anything else is
/// treated as a single-arg form.
pub(super) fn compile_args(
    value: &OwnedDataValue,
    engine: Option<&Engine>,
    templating: bool,
    ctx: &mut CompileCtx,
) -> Result<Box<[CompiledNode]>> {
    match value {
        OwnedDataValue::Array(arr) => arr
            .iter()
            .enumerate()
            .map(|(i, v)| indexed(i, ctx, |ctx| compile_node(v, engine, templating, ctx)))
            .collect::<Result<Vec<_>>>()
            .map(Vec::into_boxed_slice),
        _ => Ok(vec![compile_node(value, engine, templating, ctx)?].into_boxed_slice()),
    }
}

/// Run `compile` with object key `key` appended to the recorded pointer.
#[inline]
fn keyed<T>(key: &str, ctx: &mut CompileCtx, compile: impl FnOnce(&mut CompileCtx) -> T) -> T {
    let mark = ctx.descend(key);
    let out = compile(ctx);
    ctx.ascend(mark);
    out
}

/// Run `compile` with array index `i` appended to the recorded pointer.
#[inline]
fn indexed<T>(i: usize, ctx: &mut CompileCtx, compile: impl FnOnce(&mut CompileCtx) -> T) -> T {
    let mark = ctx.descend_index(i);
    let out = compile(ctx);
    ctx.ascend(mark);
    out
}
