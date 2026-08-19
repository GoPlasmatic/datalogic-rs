//! Object take-apart operators: `keys`, `values`, `entries`.
//!
//! The read-side complement of templating's computed-key object
//! construction: `keys`/`values` enumerate one axis of an object,
//! `entries` turns it into `[{key, value}]` rows consumable by the array
//! vocabulary (`map`, `filter`, `group_by`, ...).
//!
//! All three accept a single object argument. `null` yields `[]` (the
//! usual null-tolerant collection behavior); any other non-object input
//! is an error. Keys come out in stored order, duplicates included —
//! arena objects are plain pair slices and these operators report them
//! as-is.

use crate::arena::{ContextStack, DataValue};
use crate::{CompiledNode, Engine, Result};
use bumpalo::Bump;

/// Resolve the single object argument shared by all three operators.
/// `Ok(None)` means "result is the empty array" (null or empty object).
#[inline]
fn resolve_object<'a>(
    args: &'a [CompiledNode],
    ctx: &mut ContextStack<'a>,
    engine: &Engine,
    arena: &'a Bump,
) -> Result<Option<&'a [(&'a str, DataValue<'a>)]>> {
    if args.is_empty() {
        return Err(crate::Error::invalid_args());
    }
    match engine.dispatch_node(&args[0], ctx, arena)? {
        DataValue::Object([]) => Ok(None),
        DataValue::Object(pairs) => Ok(Some(pairs)),
        DataValue::Null => Ok(None),
        _ => Err(crate::Error::invalid_args()),
    }
}

/// `keys: [obj]` → array of the object's key strings.
#[inline]
pub(crate) fn evaluate_keys<'a>(
    args: &'a [CompiledNode],
    ctx: &mut ContextStack<'a>,
    engine: &Engine,
    arena: &'a Bump,
) -> Result<&'a DataValue<'a>> {
    let Some(pairs) = resolve_object(args, ctx, engine, arena)? else {
        return Ok(crate::arena::singletons::singleton_empty_array());
    };
    // Key strings are already arena-resident — re-borrow, no copies.
    let slice = arena.alloc_slice_fill_iter(pairs.iter().map(|(k, _)| DataValue::String(k)));
    Ok(arena.alloc(DataValue::Array(slice)))
}

/// `values: [obj]` → array of the object's values.
#[inline]
pub(crate) fn evaluate_values<'a>(
    args: &'a [CompiledNode],
    ctx: &mut ContextStack<'a>,
    engine: &Engine,
    arena: &'a Bump,
) -> Result<&'a DataValue<'a>> {
    let Some(pairs) = resolve_object(args, ctx, engine, arena)? else {
        return Ok(crate::arena::singletons::singleton_empty_array());
    };
    let slice = arena.alloc_slice_fill_iter(pairs.iter().map(|(_, v)| *v));
    Ok(arena.alloc(DataValue::Array(slice)))
}

/// `entries: [obj]` → array of `{key, value}` rows.
#[inline]
pub(crate) fn evaluate_entries<'a>(
    args: &'a [CompiledNode],
    ctx: &mut ContextStack<'a>,
    engine: &Engine,
    arena: &'a Bump,
) -> Result<&'a DataValue<'a>> {
    let Some(pairs) = resolve_object(args, ctx, engine, arena)? else {
        return Ok(crate::arena::singletons::singleton_empty_array());
    };
    let slice = arena.alloc_slice_fill_iter(pairs.iter().map(|(k, v)| {
        let row = arena.alloc([("key", DataValue::String(k)), ("value", *v)]);
        DataValue::Object(&row[..])
    }));
    Ok(arena.alloc(DataValue::Array(slice)))
}
