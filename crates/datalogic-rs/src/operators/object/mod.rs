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

use crate::Result;
use crate::arena::DataValue;
use crate::operators::eager::Cx;

/// The pairs of the single object argument shared by all three
/// operators (the row reads it as `Nullable<Obj>`: an object or `null`,
/// anything else is `InvalidArguments`). `None` means "result is the
/// empty array" (null or empty object).
///
/// Charges one operation per pair before the caller builds its result:
/// each operator produces one item per pair.
#[inline]
fn pairs_of<'a>(
    cx: &mut Cx<'_, 'a>,
    object: Option<&'a [(&'a str, DataValue<'a>)]>,
) -> Result<Option<&'a [(&'a str, DataValue<'a>)]>> {
    match object {
        None | Some([]) => Ok(None),
        Some(pairs) => {
            cx.charge(pairs.len() as u64)?;
            Ok(Some(pairs))
        }
    }
}

/// `keys: [obj]` → array of the object's key strings.
#[inline]
pub(crate) fn keys<'a>(
    cx: &mut Cx<'_, 'a>,
    object: Option<&'a [(&'a str, DataValue<'a>)]>,
) -> Result<&'a DataValue<'a>> {
    let Some(pairs) = pairs_of(cx, object)? else {
        return Ok(crate::arena::singletons::singleton_empty_array());
    };
    // Key strings are already arena-resident — re-borrow, no copies.
    let slice = cx
        .arena
        .alloc_slice_fill_iter(pairs.iter().map(|(k, _)| DataValue::String(k)));
    Ok(cx.alloc(DataValue::Array(slice)))
}

/// `values: [obj]` → array of the object's values.
#[inline]
pub(crate) fn values<'a>(
    cx: &mut Cx<'_, 'a>,
    object: Option<&'a [(&'a str, DataValue<'a>)]>,
) -> Result<&'a DataValue<'a>> {
    let Some(pairs) = pairs_of(cx, object)? else {
        return Ok(crate::arena::singletons::singleton_empty_array());
    };
    let slice = cx
        .arena
        .alloc_slice_fill_iter(pairs.iter().map(|(_, v)| *v));
    Ok(cx.alloc(DataValue::Array(slice)))
}

/// `entries: [obj]` → array of `{key, value}` rows.
#[inline]
pub(crate) fn entries<'a>(
    cx: &mut Cx<'_, 'a>,
    object: Option<&'a [(&'a str, DataValue<'a>)]>,
) -> Result<&'a DataValue<'a>> {
    let Some(pairs) = pairs_of(cx, object)? else {
        return Ok(crate::arena::singletons::singleton_empty_array());
    };
    let arena = cx.arena;
    let slice = arena.alloc_slice_fill_iter(pairs.iter().map(|(k, v)| {
        let row = arena.alloc([("key", DataValue::String(k)), ("value", *v)]);
        DataValue::Object(&row[..])
    }));
    Ok(cx.alloc(DataValue::Array(slice)))
}
