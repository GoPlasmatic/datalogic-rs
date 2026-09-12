//! `compile_node` and friends: convert an [`OwnedDataValue`] rule tree into
//! the engine's [`CompiledNode`] form.
//!
//! `compile_node` dispatches by [`OwnedDataValue`] variant. The interesting
//! case is a single-key object — that's an operator invocation, and
//! operator-specific specialisations live in `super::operator`.

use datavalue::OwnedDataValue;

use crate::node::{CompileCtx, CompiledNode, node_is_static};
use crate::opcode::OpCode;
use crate::{Engine, Result};

use super::missing::{compile_missing, compile_missing_some};
use super::operator;
use super::optimize;

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
            compile_operator_invocation(op_name, args_value, engine, templating, ctx)
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
                compile_node(val, engine, templating, ctx)
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
fn compile_operator_invocation(
    op_name: &str,
    args_value: &OwnedDataValue,
    engine: Option<&Engine>,
    templating: bool,
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

    if let Ok(opcode) = op_name.parse::<OpCode>() {
        return compile_builtin(op_name, opcode, args_value, engine, templating, ctx);
    }

    #[cfg(feature = "templating")]
    if templating {
        return compile_templating_unknown(op_name, args_value, engine, templating, ctx);
    }

    let args = compile_args(args_value, engine, templating, ctx)?;
    Ok(custom_operator_node(op_name, args, ctx))
}

/// Exactly the object datavalue's tensor serializer emits: the three keys
/// `dtype` / `shape` / `data`, no others, each holding a literal of the
/// right JSON type. Deliberately narrow — anything else keeps compiling as
/// a rule, so `{"tensor": {"val": "prediction"}}` still reads a tensor out
/// of the data rather than being mistaken for a wire body.
#[cfg(feature = "tensor")]
fn is_tensor_wire_body(fields: &[(String, OwnedDataValue)]) -> bool {
    if fields.len() != 3 {
        return false;
    }
    let get = |k: &str| fields.iter().find(|(n, _)| n == k).map(|(_, v)| v);
    matches!(get("dtype"), Some(OwnedDataValue::String(_)))
        && matches!(get("data"), Some(OwnedDataValue::String(_)))
        && matches!(get("shape"), Some(OwnedDataValue::Array(dims))
            if dims.iter().all(|d| matches!(d, OwnedDataValue::Number(_))))
}

/// Build a `CustomOperator` node from an op name and its already-compiled args.
fn custom_operator_node(
    op_name: &str,
    args: Box<[CompiledNode]>,
    ctx: &mut CompileCtx,
) -> CompiledNode {
    CompiledNode::CustomOperator(Box::new(crate::node::CustomOperatorData {
        id: Some(ctx.next_id()),
        name: op_name.to_string(),
        args,
    }))
}

/// Builtin operator path: handle invalid-args sentinels for `and`/`or`/`if`,
/// var/val/exists specialisations,
/// missing/missing_some, throw, and fall through to a generic
/// `BuiltinOperator` (with optimization + static-fold passes when an
/// `engine` is supplied).
fn compile_builtin(
    op_name: &str,
    opcode: OpCode,
    args_value: &OwnedDataValue,
    engine: Option<&Engine>,
    templating: bool,
    ctx: &mut CompileCtx,
) -> Result<CompiledNode> {
    let requires_array = matches!(opcode, OpCode::And | OpCode::Or | OpCode::If);
    if requires_array && !matches!(args_value, OwnedDataValue::Array(_)) {
        return Ok(invalid_args_marker(opcode, args_value, ctx));
    }

    // `{"tensor": {"dtype": .., "shape": [..], "data": ".."}}` is both the
    // operator call and the wire form the emitter writes, so a serialized
    // tensor pasted into a rule has to evaluate back to itself. Compiling
    // that body as a rule would fail — it is a three-key object, which is
    // an unknown operator outside templating mode — so recognise the
    // emitter's exact shape and compile it as a literal argument instead.
    #[cfg(feature = "tensor")]
    if opcode == OpCode::TensorMake
        && let OwnedDataValue::Object(fields) = args_value
        && is_tensor_wire_body(fields)
    {
        let body = CompiledNode::compile_time_value(Some(ctx.next_id()), args_value.clone());
        return Ok(CompiledNode::BuiltinOperator {
            id: Some(ctx.next_id()),
            opcode,
            args: Box::new([body]),
            predicate_hint: None,
            iter_arg_kind: crate::operators::array::IterArgKind::General,
        });
    }

    let args = compile_args(args_value, engine, templating, ctx)?;

    if let Some(node) = try_specialised(op_name, opcode, &args, args_value, ctx) {
        return Ok(node);
    }

    if opcode == OpCode::Missing {
        return Ok(compile_missing(args, ctx));
    }
    if opcode == OpCode::MissingSome {
        return Ok(compile_missing_some(args, ctx));
    }

    #[cfg(feature = "error-handling")]
    if let Some(node) = try_compile_throw_literal(opcode, &args, ctx) {
        return Ok(node);
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
    if let Some(eng) = engine {
        if !ctx.skip_fold() {
            node = optimize::optimize(node, eng);
            if node_is_static(&node) {
                if let Some(value) = optimize::constant_fold::fold_static_node(&node, eng) {
                    return Ok(CompiledNode::compile_time_value(Some(ctx.next_id()), value));
                }
            }
        }
    }

    Ok(node)
}

/// Try the operator-specific compile-time specialisations: `var`, `val`,
/// `exists`. Returns `None` if no specialisation applies.
///
/// We've already paid for `op_name -> OpCode` upstream, so dispatch on the
/// opcode first and skip the per-call string compares for the (common)
/// non-specialised path. `var` and `val` both compile to `OpCode::Val`,
/// but with different arg-shape semantics — `var`'s second arg is a
/// default fallback, `val`'s is a path-chain segment — so the inner split
/// on `op_name` stays.
fn try_specialised(
    op_name: &str,
    opcode: OpCode,
    args: &[CompiledNode],
    // Raw pre-compile arguments. Only the datetime timezone check wants
    // them, to hand `invalid_args_marker` a serialisable copy of the rule.
    #[cfg_attr(not(feature = "datetime"), allow(unused_variables))] args_value: &OwnedDataValue,
    ctx: &mut CompileCtx,
) -> Option<CompiledNode> {
    match opcode {
        OpCode::Val => {
            // Only "var" / "val" map to `OpCode::Val` (see `OpCode::FromStr`).
            if op_name == "var" {
                operator::try_compile_var(args, ctx)
            } else {
                operator::try_compile_val(args, ctx)
            }
        }
        #[cfg(feature = "ext-control")]
        OpCode::Exists => operator::try_compile_exists(args, ctx),
        #[cfg(feature = "datetime")]
        OpCode::FormatDate | OpCode::ParseDate => {
            try_validate_timezone_literal(opcode, args, args_value, ctx)
        }
        _ => None,
    }
}

/// `format_date` / `parse_date` with a *literal* timezone argument:
/// validate the zone name against chrono-tz's compiled-in table at compile
/// time, so a typo'd zone fails when the rule is built instead of on first
/// evaluation. Dynamic zone expressions still validate at evaluation.
/// Follows the [`invalid_args_marker`] precedent — the marker raises at
/// dispatch carrying the op name, keeping the breadcrumb path intact.
#[cfg(feature = "datetime")]
fn try_validate_timezone_literal(
    opcode: OpCode,
    args: &[CompiledNode],
    args_value: &OwnedDataValue,
    ctx: &mut CompileCtx,
) -> Option<CompiledNode> {
    let CompiledNode::Value {
        value: OwnedDataValue::String(s),
        ..
    } = args.get(2)?
    else {
        return None;
    };
    if s.parse::<chrono_tz::Tz>().is_ok() {
        return None;
    }
    Some(invalid_args_marker(opcode, args_value, ctx))
}

/// Build the [`CompiledNode::InvalidArgs`] placeholder for `and` / `or` /
/// `if` invoked with a non-array argument. Carries the op name forward so
/// the dispatcher can produce an error that names the failing op rather
/// than a generic "Invalid Arguments", and the raw `args_value` so
/// `to_json` can reproduce the offending rule verbatim instead of a
/// placeholder that re-parses as something else.
fn invalid_args_marker(
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

/// `throw` with a literal string argument compiles to a pre-built error
/// payload so runtime evaluation has nothing to coerce.
#[cfg(feature = "error-handling")]
fn try_compile_throw_literal(
    opcode: OpCode,
    args: &[CompiledNode],
    ctx: &mut CompileCtx,
) -> Option<CompiledNode> {
    if opcode != OpCode::Throw || args.len() != 1 {
        return None;
    }
    let CompiledNode::Value {
        value: OwnedDataValue::String(s),
        ..
    } = &args[0]
    else {
        return None;
    };
    Some(CompiledNode::Throw(Box::new(
        crate::node::CompiledThrowData {
            id: Some(ctx.next_id()),
            error: OwnedDataValue::Object(vec![(
                "type".to_string(),
                OwnedDataValue::String(s.clone()),
            )]),
        },
    )))
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
    ctx: &mut CompileCtx,
) -> Result<CompiledNode> {
    if let Some(eng) = engine {
        if eng.has_custom_operator(op_name) {
            let args = compile_args(args_value, engine, templating, ctx)?;
            return Ok(custom_operator_node(op_name, args, ctx));
        }
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
    let compiled_val = compile_node(value, engine, templating, ctx)?;
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
        .map(|v| compile_node(v, engine, templating, ctx))
        .collect::<Result<Vec<_>>>()?;

    let nodes_boxed = nodes.into_boxed_slice();
    let node = CompiledNode::Array {
        id: Some(ctx.next_id()),
        nodes: nodes_boxed,
    };

    if let Some(eng) = engine {
        if !ctx.skip_fold() && node_is_static(&node) {
            if let Some(value) = optimize::constant_fold::fold_static_node(&node, eng) {
                // `compile_time_value`: the folded array carries its
                // prebuilt composite view immediately, so an enclosing
                // static operator folded during this same compile (e.g. a
                // literal-discriminant `switch` matching its case table
                // via `lit: Some`) evaluates correctly at fold time.
                return Ok(CompiledNode::compile_time_value(Some(ctx.next_id()), value));
            }
        }
    }

    Ok(node)
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
            .map(|v| compile_node(v, engine, templating, ctx))
            .collect::<Result<Vec<_>>>()
            .map(Vec::into_boxed_slice),
        _ => Ok(vec![compile_node(value, engine, templating, ctx)?].into_boxed_slice()),
    }
}
