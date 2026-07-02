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
///
/// Two lanes:
///
/// - **Deferred** (inside a protected `try` arm, no tracer): the error is
///   guaranteed to be caught by the enclosing `try`, so the payload never
///   needs to exist in owned form — normalize it in the arena and park the
///   borrow in the context's thrown slot; the `Error` carries only a
///   placeholder. This removes the owned deep copy at the throw site *and*
///   the owned→arena deep copy at the catch site.
/// - **Eager** (may escape to the API boundary, or a tracer is attached
///   and will render the payload per step): build the owned payload as
///   before.
#[inline]
pub(crate) fn evaluate_throw<'a>(
    args: &'a [CompiledNode],
    ctx: &mut ContextStack<'a>,
    engine: &Engine,
    arena: &'a Bump,
) -> Result<&'a DataValue<'a>> {
    if ctx.in_catch_scope() && !ctx.is_tracing() {
        let av: &'a DataValue<'a> = if args.is_empty() {
            crate::arena::singletons::singleton_null()
        } else if let CompiledNode::Value { value, .. } = &args[0] {
            // Literal fast path — skip arena dispatch.
            arena.alloc(value.to_arena(arena))
        } else {
            engine.dispatch_node(&args[0], ctx, arena)?
        };
        ctx.set_thrown_slot(normalize_thrown_arena(av, arena));
        return Err(Error::deferred_thrown());
    }

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

/// Normalize a thrown arena value into the error-object shape. Must mirror
/// the owned-path normalization in [`evaluate_throw`] exactly (the
/// differential property test compares the deferred lane against the traced
/// pipeline, which uses the owned path): objects pass through borrowed,
/// strings become `{"type": s}`, everything else becomes
/// `{"type": <type name>}`.
#[inline]
fn normalize_thrown_arena<'a>(av: &'a DataValue<'a>, arena: &'a Bump) -> &'a DataValue<'a> {
    let type_val: DataValue<'a> = match av {
        DataValue::Object(_) => return av,
        DataValue::String(s) => DataValue::String(s),
        other => DataValue::String(arena_type_name(other)),
    };
    let entry = arena.alloc([("type", type_val)]);
    arena.alloc(DataValue::Object(&entry[..]))
}

/// Stable type name for a non-string, non-object thrown arena value.
/// Mirrors [`value_type_name`] (and the `type` operator's output).
#[inline]
fn arena_type_name(v: &DataValue<'_>) -> &'static str {
    match v {
        DataValue::Null => "null",
        DataValue::Bool(_) => "boolean",
        DataValue::Number(_) => "number",
        DataValue::String(_) => "string",
        DataValue::Array(_) => "array",
        DataValue::Object(_) => "object",
        #[cfg(feature = "datetime")]
        DataValue::DateTime(_) => "datetime",
        #[cfg(feature = "datetime")]
        DataValue::Duration(_) => "duration",
    }
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
        // Any error raised inside this arm is consumed by this loop, which
        // unlocks the deferred thrown-payload fast lane in `throw` / NaN
        // sites (see `evaluate_throw`). Clearing the slot first keeps a
        // stale payload — from an earlier arm here, or from an enclosing
        // `try`'s arm sequence — from pairing with an unrelated `Thrown`
        // error this arm might raise through a non-deferring site.
        ctx.clear_thrown_slot();
        ctx.enter_catch_scope();
        let result = engine.dispatch_node(arg, ctx, arena);
        ctx.exit_catch_scope();
        match result {
            Ok(v) => return Ok(v),
            Err(e) => {
                ctx.truncate_error_path(saved_len);
                last_err = Some(e);
            }
        }
    }
    Err(last_err.unwrap_or_else(Error::invalid_args))
}

/// Pushes the thrown error object onto the arena context stack as the
/// current frame so the catch arm's `var`/`val` lookups see error fields.
///
/// Payload source, in order: the context's thrown slot when the failing
/// arm deferred it (already arena-resident — no conversion), otherwise the
/// owned payload inside the error, deep-converted into the arena. A
/// literal catch arm can't read the context at all, so the push (and any
/// materialization) is skipped entirely.
#[inline]
fn try_last_with_error_context<'a>(
    arg: &'a CompiledNode,
    last_error: &mut Option<Error>,
    ctx: &mut ContextStack<'a>,
    engine: &Engine,
    arena: &'a Bump,
) -> Result<&'a DataValue<'a>> {
    // Consume the slot unconditionally: it either pairs with `last_error`
    // (set during the same failing arm) or must not leak past this catch.
    let slot = ctx.take_thrown_slot();
    if let Some(Error {
        kind: crate::ErrorKind::Thrown(error_obj),
        ..
    }) = last_error.take()
    {
        if matches!(arg, CompiledNode::Value { .. }) {
            return engine.dispatch_node(arg, ctx, arena);
        }
        let av: &'a DataValue<'a> = match slot {
            Some(av) => av,
            None => arena.alloc(error_obj.to_arena(arena)),
        };
        ctx.push(av);
        let result = engine.dispatch_node(arg, ctx, arena);
        ctx.pop();
        result
    } else {
        engine.dispatch_node(arg, ctx, arena)
    }
}
