//! Error-handling operators: `throw` and `try`.
//!
//! `throw` raises a structured error; `try` evaluates expressions in
//! sequence until one succeeds (the final arm receives the caught error
//! object as its context, so the catch body can inspect error fields via
//! `var` / `val`).
//!
//! # Syntax
//!
//! ```json
//! {"throw": "ErrorType"}
//! {"throw": {"code": 404, "message": "Not found"}}
//!
//! {"try": [expression, fallback1, fallback2, ...]}
//!
//! {"try": [
//!   {"throw": {"code": 404, "message": "Not found"}},
//!   {"cat": ["Error: ", {"var": "message"}]}
//! ]}
//! // Returns: "Error: Not found"
//! ```

use datavalue::OwnedDataValue;

use crate::arena::{ContextStack, DataValue};
use crate::{CompiledNode, Engine, Error, Result};
use bumpalo::Bump;

// ─── throw ──────────────────────────────────────────────────────────────────

/// `throw`. Builds the error object directly from the argument's arena form.
#[inline]
pub(crate) fn evaluate_throw<'a>(
    args: &'a [CompiledNode],
    ctx: &mut ContextStack<'a>,
    engine: &Engine,
    arena: &'a Bump,
) -> Result<&'a DataValue<'a>> {
    let owned: OwnedDataValue = if args.is_empty() {
        OwnedDataValue::Null
    } else if let CompiledNode::Value { value, .. } = &args[0] {
        // Literal fast path — skip arena dispatch.
        value.clone()
    } else {
        let av = engine.dispatch_node(&args[0], ctx, arena)?;
        av.to_owned()
    };

    let owned = match owned {
        OwnedDataValue::Object(_) => owned,
        OwnedDataValue::String(s) => OwnedDataValue::object([("type", s)]),
        // Scalar / array — wrap in `{type: <name>}` using stable type names
        // (matches the `type` operator's output). `Debug` formatting was
        // not API-stable and revealed `OwnedDataValue` variant constructors.
        ref other => OwnedDataValue::object([("type", value_type_name(other))]),
    };

    Err(Error::thrown(owned))
}

/// Stable type name for a non-string, non-object thrown value. Mirrors the
/// names produced by the `type` operator (`operators::inspect`).
#[inline]
fn value_type_name(v: &OwnedDataValue) -> &'static str {
    match v {
        OwnedDataValue::Null => "null",
        OwnedDataValue::Bool(_) => "boolean",
        OwnedDataValue::Number(_) => "number",
        OwnedDataValue::String(_) => "string",
        OwnedDataValue::Array(_) => "array",
        OwnedDataValue::Object(_) => "object",
        #[cfg(feature = "datetime")]
        OwnedDataValue::DateTime(_) => "datetime",
        #[cfg(feature = "datetime")]
        OwnedDataValue::Duration(_) => "duration",
    }
}

// ─── try ────────────────────────────────────────────────────────────────────

#[inline]
pub(crate) fn evaluate_try<'a>(
    args: &'a [CompiledNode],
    ctx: &mut ContextStack<'a>,
    engine: &Engine,
    arena: &'a Bump,
) -> Result<&'a DataValue<'a>> {
    if args.is_empty() {
        return Ok(crate::arena::singletons::singleton_null());
    }
    if args.len() == 1 {
        return engine.dispatch_node(&args[0], ctx, arena);
    }

    // Multi-arg form: try arms in sequence; final arm receives the error
    // object as its context.
    let last_idx = args.len() - 1;
    let mut last_err: Option<Error> = None;
    for (i, arg) in args.iter().enumerate() {
        if i == last_idx {
            return try_last_with_error_context(arg, &mut last_err, ctx, engine, arena);
        }
        let saved_len = ctx.error_path_len();
        match engine.dispatch_node(arg, ctx, arena) {
            Ok(v) => return Ok(v),
            Err(e) => {
                ctx.truncate_error_path(saved_len);
                last_err = Some(e);
            }
        }
    }
    Err(last_err.unwrap_or_else(|| Error::invalid_arguments(crate::error::INVALID_ARGS)))
}

/// Pushes the thrown error object onto the arena context stack as the
/// current frame so the catch arm's `var`/`val` lookups see error fields.
#[inline]
fn try_last_with_error_context<'a>(
    arg: &'a CompiledNode,
    last_error: &mut Option<Error>,
    ctx: &mut ContextStack<'a>,
    engine: &Engine,
    arena: &'a Bump,
) -> Result<&'a DataValue<'a>> {
    if let Some(Error {
        kind: crate::ErrorKind::Thrown(error_obj),
        ..
    }) = last_error.take()
    {
        let av: &'a DataValue<'a> = arena.alloc(error_obj.to_arena(arena));
        ctx.push(av);
        let result = engine.dispatch_node(arg, ctx, arena);
        ctx.pop();
        result
    } else {
        engine.dispatch_node(arg, ctx, arena)
    }
}
