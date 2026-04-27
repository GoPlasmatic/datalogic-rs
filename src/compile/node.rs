//! `compile_node` and friends: convert an [`OwnedDataValue`] rule tree into
//! the engine's [`CompiledNode`] form.
//!
//! `compile_node` dispatches by [`OwnedDataValue`] variant. The interesting
//! case is a single-key object — that's an operator invocation, and
//! operator-specific specialisations live in `super::operator`.

use datavalue::OwnedDataValue;

use crate::node::{CompileCtx, CompiledNode, node_is_static};
use crate::opcode::OpCode;
use crate::{DataLogic, Result};

use super::missing::{compile_missing, compile_missing_some};
use super::operator;
use super::optimize;

/// Compile a single value into a [`CompiledNode`].
pub(super) fn compile_node(
    value: &OwnedDataValue,
    engine: Option<&DataLogic>,
    preserve_structure: bool,
    ctx: &mut CompileCtx,
) -> Result<CompiledNode> {
    match value {
        OwnedDataValue::Object(pairs) if pairs.len() > 1 => {
            compile_multi_key_object(pairs, engine, preserve_structure, ctx)
        }
        OwnedDataValue::Object(pairs) if pairs.len() == 1 => {
            let (op_name, args_value) = &pairs[0];
            compile_operator_invocation(op_name, args_value, engine, preserve_structure, ctx)
        }
        OwnedDataValue::Array(arr) => compile_array(arr, engine, preserve_structure, ctx),
        _ => Ok(CompiledNode::value_with_id(ctx.next_id(), value.clone())),
    }
}

/// Multi-key object — only valid in `preserve_structure` mode (where it
/// becomes a structured-object output template); otherwise an error.
fn compile_multi_key_object(
    pairs: &[(String, OwnedDataValue)],
    engine: Option<&DataLogic>,
    preserve_structure: bool,
    ctx: &mut CompileCtx,
) -> Result<CompiledNode> {
    #[cfg(feature = "preserve")]
    if preserve_structure {
        let fields: Vec<_> = pairs
            .iter()
            .map(|(key, val)| {
                compile_node(val, engine, preserve_structure, ctx)
                    .map(|compiled_val| (key.clone(), compiled_val))
            })
            .collect::<Result<Vec<_>>>()?;
        return Ok(CompiledNode::StructuredObject(Box::new(
            crate::node::StructuredObjectData {
                id: ctx.next_id(),
                fields: fields.into_boxed_slice(),
            },
        )));
    }
    let _ = (pairs, engine, preserve_structure, ctx);
    Err(crate::error::Error::InvalidOperator(
        "Unknown Operator".to_string(),
    ))
}

/// Single-key object: an operator invocation. Routes to either the builtin
/// path (when the key parses as an `OpCode`) or the custom-operator /
/// preserve-structure path.
fn compile_operator_invocation(
    op_name: &str,
    args_value: &OwnedDataValue,
    engine: Option<&DataLogic>,
    preserve_structure: bool,
    ctx: &mut CompileCtx,
) -> Result<CompiledNode> {
    if let Ok(opcode) = op_name.parse::<OpCode>() {
        return compile_builtin(op_name, opcode, args_value, engine, preserve_structure, ctx);
    }

    #[cfg(feature = "preserve")]
    if preserve_structure {
        return compile_preserve_unknown(op_name, args_value, engine, preserve_structure, ctx);
    }

    let args = compile_args(args_value, engine, preserve_structure, ctx)?;
    Ok(CompiledNode::CustomOperator(Box::new(
        crate::node::CustomOperatorData {
            id: ctx.next_id(),
            name: op_name.to_string(),
            args,
        },
    )))
}

/// Builtin operator path: handle invalid-args sentinels for `and`/`or`/`if`,
/// preserve-args for `Preserve`, var/val/exists specialisations,
/// missing/missing_some, throw, and fall through to a generic
/// `BuiltinOperator` (with optimization + static-fold passes when an
/// `engine` is supplied).
fn compile_builtin(
    op_name: &str,
    opcode: OpCode,
    args_value: &OwnedDataValue,
    engine: Option<&DataLogic>,
    preserve_structure: bool,
    ctx: &mut CompileCtx,
) -> Result<CompiledNode> {
    let requires_array = matches!(opcode, OpCode::And | OpCode::Or | OpCode::If);
    if requires_array && !matches!(args_value, OwnedDataValue::Array(_)) {
        return Ok(invalid_args_marker(opcode, args_value, ctx));
    }

    let args = compile_builtin_args(opcode, args_value, engine, preserve_structure, ctx)?;

    if let Some(node) = try_specialised(op_name, opcode, &args, ctx) {
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
        id: ctx.next_id(),
        opcode,
        args,
    };

    // Optimization + static-fold passes (engine-dependent).
    if let Some(eng) = engine {
        node = optimize::optimize(node, eng);
        if node_is_static(&node)
            && let Some(value) = optimize::constant_fold::fold_static_node(&node, eng)
        {
            return Ok(CompiledNode::value_with_id(ctx.next_id(), value));
        }
    }

    Ok(node)
}

/// Compile the `args_value` into a slice of [`CompiledNode`], honoring
/// `Preserve`'s special "raw values, not compiled logic" rule.
fn compile_builtin_args(
    opcode: OpCode,
    args_value: &OwnedDataValue,
    engine: Option<&DataLogic>,
    preserve_structure: bool,
    ctx: &mut CompileCtx,
) -> Result<Box<[CompiledNode]>> {
    #[cfg(feature = "preserve")]
    if opcode == OpCode::Preserve {
        let nodes = match args_value {
            OwnedDataValue::Array(arr) => arr
                .iter()
                .map(|v| CompiledNode::value_with_id(ctx.next_id(), v.clone()))
                .collect::<Vec<_>>(),
            _ => vec![CompiledNode::value_with_id(
                ctx.next_id(),
                args_value.clone(),
            )],
        };
        return Ok(nodes.into_boxed_slice());
    }
    let _ = opcode;
    compile_args(args_value, engine, preserve_structure, ctx)
}

/// Try the operator-specific compile-time specialisations: `var`, `val`,
/// `exists`. Returns `None` if no specialisation applies.
///
/// `var` and `val` both compile to `CompiledVar`, but with different arg
/// shape semantics — `var`'s second arg is a default fallback, `val`'s
/// is a path-chain segment. We dispatch on the source operator name to
/// keep those semantics distinct, even though both map to `OpCode::Val`.
fn try_specialised(
    op_name: &str,
    opcode: OpCode,
    args: &[CompiledNode],
    ctx: &mut CompileCtx,
) -> Option<CompiledNode> {
    if op_name == "var"
        && let Some(node) = operator::try_compile_var(args, ctx)
    {
        return Some(node);
    }
    if op_name == "val"
        && let Some(node) = operator::try_compile_val(args, ctx)
    {
        return Some(node);
    }
    #[cfg(feature = "ext-control")]
    {
        if opcode == OpCode::Exists
            && let Some(node) = operator::try_compile_exists(args, ctx)
        {
            return Some(node);
        }
    }
    let _ = opcode;
    None
}

/// Build a sentinel "invalid args" node for `and`/`or`/`if` invoked with a
/// non-array argument. The sentinel is detected at runtime to surface a
/// helpful error.
fn invalid_args_marker(
    opcode: OpCode,
    args_value: &OwnedDataValue,
    ctx: &mut CompileCtx,
) -> CompiledNode {
    let invalid_value = OwnedDataValue::Object(vec![
        ("__invalid_args__".to_string(), OwnedDataValue::Bool(true)),
        ("value".to_string(), args_value.clone()),
    ]);
    let value_node = CompiledNode::value_with_id(ctx.next_id(), invalid_value);
    let args = vec![value_node].into_boxed_slice();
    CompiledNode::BuiltinOperator {
        id: ctx.next_id(),
        opcode,
        args,
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
    Some(CompiledNode::CompiledThrow(Box::new(
        crate::node::CompiledThrowData {
            id: ctx.next_id(),
            error: OwnedDataValue::Object(vec![(
                "type".to_string(),
                OwnedDataValue::String(s.clone()),
            )]),
            arena_error: None,
        },
    )))
}

/// Unknown-operator handling under `preserve_structure` mode. Custom
/// operators registered on the engine compile to a `CustomOperator`;
/// otherwise the key/value pair becomes a single-field structured-object
/// output template.
#[cfg(feature = "preserve")]
fn compile_preserve_unknown(
    op_name: &str,
    args_value: &OwnedDataValue,
    engine: Option<&DataLogic>,
    preserve_structure: bool,
    ctx: &mut CompileCtx,
) -> Result<CompiledNode> {
    if let Some(eng) = engine
        && eng.has_custom_operator(op_name)
    {
        let args = compile_args(args_value, engine, preserve_structure, ctx)?;
        return Ok(CompiledNode::CustomOperator(Box::new(
            crate::node::CustomOperatorData {
                id: ctx.next_id(),
                name: op_name.to_string(),
                args,
            },
        )));
    }
    let compiled_val = compile_node(args_value, engine, preserve_structure, ctx)?;
    let fields = vec![(op_name.to_string(), compiled_val)].into_boxed_slice();
    Ok(CompiledNode::StructuredObject(Box::new(
        crate::node::StructuredObjectData {
            id: ctx.next_id(),
            fields,
        },
    )))
}

/// Compile a literal array. When all elements are static and an engine is
/// supplied, the whole array is constant-folded to an [`OwnedDataValue`] literal.
fn compile_array(
    arr: &[OwnedDataValue],
    engine: Option<&DataLogic>,
    preserve_structure: bool,
    ctx: &mut CompileCtx,
) -> Result<CompiledNode> {
    let nodes = arr
        .iter()
        .map(|v| compile_node(v, engine, preserve_structure, ctx))
        .collect::<Result<Vec<_>>>()?;

    let nodes_boxed = nodes.into_boxed_slice();
    let node = CompiledNode::Array {
        id: ctx.next_id(),
        nodes: nodes_boxed,
    };

    if let Some(eng) = engine
        && node_is_static(&node)
        && let Some(value) = optimize::constant_fold::fold_static_node(&node, eng)
    {
        return Ok(CompiledNode::value_with_id(ctx.next_id(), value));
    }

    Ok(node)
}

/// Compile operator arguments — an array is iterated; anything else is
/// treated as a single-arg form.
pub(super) fn compile_args(
    value: &OwnedDataValue,
    engine: Option<&DataLogic>,
    preserve_structure: bool,
    ctx: &mut CompileCtx,
) -> Result<Box<[CompiledNode]>> {
    match value {
        OwnedDataValue::Array(arr) => arr
            .iter()
            .map(|v| compile_node(v, engine, preserve_structure, ctx))
            .collect::<Result<Vec<_>>>()
            .map(Vec::into_boxed_slice),
        _ => Ok(vec![compile_node(value, engine, preserve_structure, ctx)?].into_boxed_slice()),
    }
}
