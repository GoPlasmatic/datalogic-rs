//! Compile hooks: the specialised compiled forms operator table rows
//! declare with `compile: Some(..)`.
//!
//! The walker makes one call per builtin (see `walker::compile_builtin`):
//! a [`CompileHook::Raw`] hook sees the uncompiled argument value, a
//! [`CompileHook::Args`] hook the compiled arguments. A hook that does not
//! apply hands the arguments back ([`Hooked::Generic`]) and the operator
//! compiles to a plain `BuiltinOperator` node.
//!
//! [`CompileHook::Raw`]: crate::operators::meta::CompileHook::Raw
//! [`CompileHook::Args`]: crate::operators::meta::CompileHook::Args

// Which variants, extractors and adapters a build uses depends on which
// operator families it compiles in; with every family on, all of them are
// used and dead code is still reported.
#![cfg_attr(not(feature = "all-operators"), allow(dead_code, unused_imports))]

use crate::node::{CompileCtx, CompiledNode};
use crate::operators::meta::{HookArgs, Hooked};
#[cfg(any(feature = "error-handling", feature = "tensor", feature = "datetime"))]
use datavalue::OwnedDataValue;

use super::missing::{compile_missing, compile_missing_some};
use super::operator;

/// Keep the arguments for the generic path when `node` is `None`.
#[inline]
fn or_generic(node: Option<CompiledNode>, args: Box<[CompiledNode]>) -> Hooked {
    match node {
        Some(node) => Hooked::Node(node),
        None => Hooked::Generic(args),
    }
}

/// `val` / `var` → `CompiledNode::Var` with pre-parsed segments. Both
/// names map to `OpCode::Val`, but `var`'s second argument is a default
/// and `val`'s is a path segment, so the source name picks the form.
pub(crate) fn val(args: Box<[CompiledNode]>, hook: HookArgs<'_>) -> Hooked {
    let node = if hook.op_name == "var" {
        operator::try_compile_var(&args, hook.ctx)
    } else {
        operator::try_compile_val(&args, hook.ctx)
    };
    or_generic(node, args)
}

/// `exists` → `CompiledNode::Exists` with pre-parsed segments.
#[cfg(feature = "ext-control")]
pub(crate) fn exists(args: Box<[CompiledNode]>, hook: HookArgs<'_>) -> Hooked {
    let node = operator::try_compile_exists(&args, hook.ctx);
    or_generic(node, args)
}

/// `missing` → `CompiledNode::Missing`, literal paths pre-parsed.
pub(crate) fn missing(args: Box<[CompiledNode]>, hook: HookArgs<'_>) -> Hooked {
    Hooked::Node(compile_missing(args, hook.ctx))
}

/// `missing_some` → `CompiledNode::MissingSome`, literal paths pre-parsed.
pub(crate) fn missing_some(args: Box<[CompiledNode]>, hook: HookArgs<'_>) -> Hooked {
    Hooked::Node(compile_missing_some(args, hook.ctx))
}

/// `throw` with a literal string argument compiles to a pre-built error
/// payload so runtime evaluation has nothing to coerce.
#[cfg(feature = "error-handling")]
pub(crate) fn throw_literal(args: Box<[CompiledNode]>, hook: HookArgs<'_>) -> Hooked {
    let [
        CompiledNode::Value {
            value: OwnedDataValue::String(s),
            ..
        },
    ] = &*args
    else {
        return Hooked::Generic(args);
    };
    Hooked::Node(CompiledNode::Throw(Box::new(
        crate::node::CompiledThrowData {
            id: Some(hook.ctx.next_id()),
            error: OwnedDataValue::Object(vec![(
                "type".to_string(),
                OwnedDataValue::String(s.clone()),
            )]),
        },
    )))
}

/// `format_date` / `parse_date` with a *literal* timezone argument:
/// validate the zone name against chrono-tz's compiled-in table at compile
/// time, so a typo'd zone fails when the rule is built instead of on first
/// evaluation. Dynamic zone expressions still validate at evaluation.
/// The marker raises at dispatch carrying the op name, keeping the
/// breadcrumb path intact.
#[cfg(feature = "datetime")]
pub(crate) fn timezone_literal(args: Box<[CompiledNode]>, hook: HookArgs<'_>) -> Hooked {
    let Some(CompiledNode::Value {
        value: OwnedDataValue::String(s),
        ..
    }) = args.get(2)
    else {
        return Hooked::Generic(args);
    };
    if s.parse::<chrono_tz::Tz>().is_ok() {
        return Hooked::Generic(args);
    }
    Hooked::Node(super::walker::invalid_args_marker(
        hook.opcode,
        hook.args_value,
        hook.ctx,
    ))
}

/// `{"tensor": {"dtype": .., "shape": [..], "data": ".."}}` is both the
/// operator call and the wire form the emitter writes, so a serialized
/// tensor pasted into a rule has to evaluate back to itself. Compiling
/// that body as a rule would fail (it is a three-key object, an unknown
/// operator outside templating mode), so recognise the emitter's exact
/// shape and compile it as a literal argument instead.
#[cfg(feature = "tensor")]
pub(crate) fn tensor_wire_body(
    args_value: &OwnedDataValue,
    ctx: &mut CompileCtx,
) -> Option<CompiledNode> {
    let OwnedDataValue::Object(fields) = args_value else {
        return None;
    };
    if !is_tensor_wire_body(fields) {
        return None;
    }
    let body = CompiledNode::compile_time_value(Some(ctx.next_id()), args_value.clone());
    Some(CompiledNode::BuiltinOperator {
        id: Some(ctx.next_id()),
        opcode: crate::OpCode::TensorMake,
        args: Box::new([body]),
        predicate_hint: None,
        iter_arg_kind: crate::operators::array::IterArgKind::General,
    })
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
