use crate::{CompiledNode, Engine, Result};
// =============================================================================
// Arena-mode string operators
// =============================================================================
//
// Pre-evaluate args via `dispatch_node` (so var lookups borrow), then
// build the result. For string-producing ops, the result is allocated as
// `&'a str` in the arena via `arena.alloc_str` — no heap `String`.

use crate::arena::{ContextStack, DataValue, data_to_str};
use bumpalo::Bump;

#[inline]
pub(crate) fn evaluate_concat<'a>(
    args: &'a [CompiledNode],
    ctx: &mut ContextStack<'a>,
    engine: &Engine,
    arena: &'a Bump,
) -> Result<&'a DataValue<'a>> {
    // Build the concatenated string using a bumpalo String to avoid heap alloc.
    let mut buf = bumpalo::collections::String::new_in(arena);
    for arg in args {
        let av = engine.dispatch_node(arg, ctx, arena)?;
        match av {
            // For arrays, concat each item's string form.
            DataValue::Array(items) => {
                for it in *items {
                    buf.push_str(data_to_str(it, arena));
                }
            }
            _ => buf.push_str(data_to_str(av, arena)),
        }
    }
    Ok(arena.alloc(DataValue::String(buf.into_bump_str())))
}

/// `substr` — char-indexed substring extraction. Negative start counts from
/// end; negative length is treated as an end position.
#[inline]
pub(crate) fn evaluate_substr<'a>(
    args: &'a [CompiledNode],
    ctx: &mut ContextStack<'a>,
    engine: &Engine,
    arena: &'a Bump,
) -> Result<&'a DataValue<'a>> {
    if args.is_empty() {
        return Ok(crate::arena::singletons::singleton_empty_string());
    }

    let s_av = engine.dispatch_node(&args[0], ctx, arena)?;
    let string = data_to_str(s_av, arena);
    let char_count = string.chars().count();

    // `start` defaults to 0; `length` is optional. Both swallow non-numeric
    // values silently (per substr's spec). Literal fast path skips dispatch.
    let start: i64 = substr_arg_i64(args.get(1), ctx, engine, arena)?.unwrap_or(0);
    let length: Option<i64> = substr_arg_i64(args.get(2), ctx, engine, arena)?;

    let actual_start = if start < 0 {
        let abs_start = start.saturating_abs() as usize;
        char_count.saturating_sub(abs_start)
    } else {
        (start as usize).min(char_count)
    };

    // How many chars to take after skipping `actual_start`; `None` = take
    // the rest. Equivalent to the previous per-branch `.collect()` logic.
    let take: Option<usize> = match length {
        // Negative length = end position from the end of the string.
        Some(len) if len < 0 => {
            let abs_end = len.saturating_abs() as usize;
            let end_pos = char_count.saturating_sub(abs_end);
            Some(end_pos.saturating_sub(actual_start))
        }
        Some(0) => Some(0),
        Some(len) => Some((len as usize).min(char_count.saturating_sub(actual_start))),
        None => None,
    };

    // The selected chars form one contiguous run of `string`, and `string`
    // is already arena-resident (`data_to_str` returns `&'a str`), so the
    // result is a borrowed sub-slice: walk to the char boundaries and slice,
    // no copy.
    let byte_start = char_to_byte_offset(string, actual_start);
    let byte_end = match take {
        Some(n) => byte_start + char_to_byte_offset(&string[byte_start..], n),
        None => string.len(),
    };

    let result = &string[byte_start..byte_end];
    if result.is_empty() {
        return Ok(crate::arena::singletons::singleton_empty_string());
    }
    Ok(arena.alloc(DataValue::String(result)))
}

/// Byte offset of the `n`-th char of `s`, or `s.len()` when `n` is at or
/// past the end. `char_indices` yields char boundaries only, so the offset
/// is always safe to slice at.
#[inline]
pub(crate) fn char_to_byte_offset(s: &str, n: usize) -> usize {
    if n == 0 {
        return 0;
    }
    s.char_indices().nth(n).map_or(s.len(), |(b, _)| b)
}

/// Resolve a substr `start` / `length` argument as `Option<i64>`. Literal
/// `Value` nodes skip the dispatch hop; everything else dispatches and
/// reads `as_i64()`. Non-numeric resolved values map to `None` (substr
/// silently swallows them — different from `slice` which errors on NaN).
#[inline]
fn substr_arg_i64<'a>(
    arg: Option<&'a CompiledNode>,
    ctx: &mut ContextStack<'a>,
    engine: &Engine,
    arena: &'a Bump,
) -> Result<Option<i64>> {
    let Some(node) = arg else { return Ok(None) };
    if let CompiledNode::Value { value, .. } = node {
        return Ok(value.as_i64());
    }
    Ok(engine.dispatch_node(node, ctx, arena)?.as_i64())
}

/// Native arena-mode `in` — checks whether a needle is contained in a
/// haystack.
///
/// The two haystack shapes intentionally use different equality:
/// - **String**: byte-level `str::contains` — matches the JSONLogic
///   spec's substring semantics. The needle must be a string; numeric /
///   bool needles never match (no implicit coercion).
/// - **Array**: per-element `compare_equals(strict=true)` — same strict
///   equality `===` uses, so `[1] in [[1], [2]]` is `true` but
///   `1 in ["1"]` is `false`.
#[inline]
pub(crate) fn evaluate_in<'a>(
    args: &'a [CompiledNode],
    ctx: &mut ContextStack<'a>,
    engine: &Engine,
    arena: &'a Bump,
) -> Result<&'a DataValue<'a>> {
    if args.len() < 2 {
        return Ok(crate::arena::singletons::singleton_false());
    }
    let needle = engine.dispatch_node(&args[0], ctx, arena)?;
    let haystack = engine.dispatch_node(&args[1], ctx, arena)?;

    let result = match haystack {
        // String haystack — substring check (needle must be a string).
        DataValue::String(h) => match needle {
            DataValue::String(n) => h.contains(*n),
            _ => false,
        },
        // Array haystack — element-equality check via arena-native
        // strict-equals.
        DataValue::Array(items) => items.iter().any(|it| {
            crate::operators::comparison::compare_equals(it, needle, true, engine).unwrap_or(false)
        }),
        _ => false,
    };
    Ok(crate::arena::singletons::singleton_bool(result))
}

#[cfg(feature = "ext-string")]
#[inline]
pub(crate) fn evaluate_starts_with<'a>(
    args: &'a [CompiledNode],
    ctx: &mut ContextStack<'a>,
    engine: &Engine,
    arena: &'a Bump,
) -> Result<&'a DataValue<'a>> {
    if args.len() < 2 {
        return Err(crate::Error::invalid_args());
    }
    let s = engine.dispatch_node(&args[0], ctx, arena)?;
    let p = engine.dispatch_node(&args[1], ctx, arena)?;
    let s_str = data_to_str(s, arena);
    let p_str = data_to_str(p, arena);
    Ok(crate::arena::singletons::singleton_bool(
        s_str.starts_with(p_str),
    ))
}

#[cfg(feature = "ext-string")]
#[inline]
pub(crate) fn evaluate_ends_with<'a>(
    args: &'a [CompiledNode],
    ctx: &mut ContextStack<'a>,
    engine: &Engine,
    arena: &'a Bump,
) -> Result<&'a DataValue<'a>> {
    if args.len() < 2 {
        return Err(crate::Error::invalid_args());
    }
    let s = engine.dispatch_node(&args[0], ctx, arena)?;
    let p = engine.dispatch_node(&args[1], ctx, arena)?;
    let s_str = data_to_str(s, arena);
    let p_str = data_to_str(p, arena);
    Ok(crate::arena::singletons::singleton_bool(
        s_str.ends_with(p_str),
    ))
}

#[cfg(feature = "ext-string")]
#[inline]
pub(crate) fn evaluate_upper<'a>(
    args: &'a [CompiledNode],
    ctx: &mut ContextStack<'a>,
    engine: &Engine,
    arena: &'a Bump,
) -> Result<&'a DataValue<'a>> {
    if args.is_empty() {
        return Err(crate::Error::invalid_args());
    }
    let av = engine.dispatch_node(&args[0], ctx, arena)?;
    let s = data_to_str(av, arena);
    // Build the upper-cased text straight into the arena instead of
    // allocating a heap `String` via `to_uppercase()` then copying it in.
    // Pre-size to the source byte length so the common (ASCII, length-
    // preserving) case never re-grows the arena buffer.
    let mut buf = bumpalo::collections::String::with_capacity_in(s.len(), arena);
    for c in s.chars() {
        for u in c.to_uppercase() {
            buf.push(u);
        }
    }
    Ok(arena.alloc(DataValue::String(buf.into_bump_str())))
}

#[cfg(feature = "ext-string")]
#[inline]
pub(crate) fn evaluate_lower<'a>(
    args: &'a [CompiledNode],
    ctx: &mut ContextStack<'a>,
    engine: &Engine,
    arena: &'a Bump,
) -> Result<&'a DataValue<'a>> {
    if args.is_empty() {
        return Err(crate::Error::invalid_args());
    }
    let av = engine.dispatch_node(&args[0], ctx, arena)?;
    let s = data_to_str(av, arena);
    // Build the lower-cased text straight into the arena instead of
    // allocating a heap `String` via `to_lowercase()` then copying it in.
    // Pre-size to the source byte length so the common (ASCII, length-
    // preserving) case never re-grows the arena buffer.
    let mut buf = bumpalo::collections::String::with_capacity_in(s.len(), arena);
    for c in s.chars() {
        for l in c.to_lowercase() {
            buf.push(l);
        }
    }
    Ok(arena.alloc(DataValue::String(buf.into_bump_str())))
}

#[cfg(feature = "ext-string")]
#[inline]
pub(crate) fn evaluate_trim<'a>(
    args: &'a [CompiledNode],
    ctx: &mut ContextStack<'a>,
    engine: &Engine,
    arena: &'a Bump,
) -> Result<&'a DataValue<'a>> {
    if args.is_empty() {
        return Err(crate::Error::invalid_args());
    }
    let av = engine.dispatch_node(&args[0], ctx, arena)?;
    let s = data_to_str(av, arena);
    // `s` is already arena-resident, so `s.trim()` is an arena `&'a str`
    // sub-slice; no re-copy needed.
    Ok(arena.alloc(DataValue::String(s.trim())))
}

/// Native arena-mode `split`. Splits text by a plain string delimiter,
/// building the result directly in the arena.
#[cfg(feature = "ext-string")]
#[inline]
pub(crate) fn evaluate_split<'a>(
    args: &'a [CompiledNode],
    ctx: &mut ContextStack<'a>,
    engine: &Engine,
    arena: &'a Bump,
) -> Result<&'a DataValue<'a>> {
    if args.len() < 2 {
        return Err(crate::Error::invalid_args());
    }
    let text_av = engine.dispatch_node(&args[0], ctx, arena)?;
    let text_str: &'a str = match text_av {
        DataValue::String(s) => s,
        _ => data_to_str(text_av, arena),
    };

    // Resolve the delimiter as a string. Literal-string fast path skips dispatch.
    let delim_str: &'a str = if let CompiledNode::Value {
        value: datavalue::OwnedDataValue::String(s),
        ..
    } = &args[1]
    {
        arena.alloc_str(s)
    } else {
        let av = engine.dispatch_node(&args[1], ctx, arena)?;
        match av {
            DataValue::String(s) => s,
            _ => data_to_str(av, arena),
        }
    };

    split_arena_normal(text_str, delim_str, arena)
}

#[cfg(feature = "ext-string")]
#[inline]
fn split_arena_normal<'a>(text: &str, delim: &str, arena: &'a Bump) -> Result<&'a DataValue<'a>> {
    if text.is_empty() {
        // Empty input → [""].
        let item: &'a str = "";
        let slice = bumpalo::vec![in arena; DataValue::String(item)].into_bump_slice();
        return Ok(arena.alloc(DataValue::Array(slice)));
    }
    if delim.is_empty() {
        // Empty delimiter → split into individual characters.
        let mut items: bumpalo::collections::Vec<'a, DataValue<'a>> =
            bumpalo::collections::Vec::with_capacity_in(text.chars().count(), arena);
        for c in text.chars() {
            // Per-char arena string. For ASCII, a 1-byte alloc per char.
            let mut buf = bumpalo::collections::String::new_in(arena);
            buf.push(c);
            items.push(DataValue::String(buf.into_bump_str()));
        }
        return Ok(arena.alloc(DataValue::Array(items.into_bump_slice())));
    }
    let mut items: bumpalo::collections::Vec<'a, DataValue<'a>> =
        bumpalo::collections::Vec::new_in(arena);
    for part in text.split(delim) {
        items.push(DataValue::String(arena.alloc_str(part)));
    }
    Ok(arena.alloc(DataValue::Array(items.into_bump_slice())))
}
