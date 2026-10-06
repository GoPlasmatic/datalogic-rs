// =============================================================================
// String operators
// =============================================================================
//
// Every operator here is an `eager` row: its arguments arrive evaluated and
// coerced. For string-producing ops, the result is allocated as `&'a str`
// in the arena, never a heap `String`.

use super::eager::Cx;
use super::extract::{Any, RestArgs};
use crate::Result;
use crate::arena::{DataValue, data_to_str};
#[cfg(feature = "ext-string")]
use {crate::arena::ContextStack, bumpalo::Bump};

/// `cat`: the string forms of every argument, concatenated; an array
/// argument contributes each item's string form.
#[inline]
pub(crate) fn concat<'a>(
    cx: &mut Cx<'_, 'a>,
    parts: RestArgs<'a, Any>,
) -> Result<&'a DataValue<'a>> {
    // Build the concatenated string using a bumpalo String to avoid heap alloc.
    // Each piece is charged its bytes before it is appended, and an array
    // argument one per item on top, so an accumulator (`cat` of the
    // accumulator inside `reduce`) is charged for the copy it makes.
    let arena = cx.arena;
    let mut buf = bumpalo::collections::String::new_in(arena);
    for i in 0..parts.len() {
        match parts.get(i, cx)? {
            // For arrays, concat each item's string form.
            DataValue::Array(items) => {
                cx.charge(items.len() as u64)?;
                for it in *items {
                    let piece = data_to_str(it, arena);
                    cx.charge_bytes(piece.len())?;
                    buf.push_str(piece);
                }
            }
            av => {
                let piece = data_to_str(av, arena);
                cx.charge_bytes(piece.len())?;
                buf.push_str(piece);
            }
        }
    }
    Ok(cx.alloc(DataValue::String(buf.into_bump_str())))
}

/// `substr(text, start?, length?)`: char-indexed substring. Negative
/// `start` counts from the end; negative `length` is an end position
/// counted from the end. A `start` or `length` that is not an integer is
/// treated as absent (per substr's spec).
#[inline]
pub(crate) fn substr<'a>(
    cx: &mut Cx<'_, 'a>,
    string: &'a str,
    start: Option<i64>,
    length: Option<i64>,
) -> Result<&'a str> {
    // Finding the boundaries scans the string.
    cx.charge_bytes(string.len())?;
    let start = start.unwrap_or(0);

    // The selected chars form one contiguous run of `string`, and `string`
    // is already arena-resident (`data_to_str` returns `&'a str`), so the
    // result is a borrowed sub-slice of it either way: find the two char
    // boundaries and slice, no copy.
    //
    // ASCII fast path: one byte per char, so both boundaries are plain
    // index math. `is_ascii` is a word-at-a-time scan — on large strings
    // roughly two orders of magnitude cheaper than the per-char decode
    // walks the general path needs.
    let (byte_start, byte_end) = if string.is_ascii() {
        let n = string.len();
        let byte_start = if start < 0 {
            n.saturating_sub(start.saturating_abs() as usize)
        } else {
            (start as usize).min(n)
        };
        let byte_end = match length {
            // Negative length = end position from the end of the string.
            Some(len) if len < 0 => n
                .saturating_sub(len.saturating_abs() as usize)
                .max(byte_start),
            Some(len) => byte_start.saturating_add(len as usize).min(n),
            None => n,
        };
        (byte_start, byte_end)
    } else {
        // `char_count` (a full decode walk) is only needed to anchor
        // negative offsets; positive offsets clamp inside
        // `char_to_byte_offset` without it.
        let char_count = if start < 0 || matches!(length, Some(len) if len < 0) {
            string.chars().count()
        } else {
            0
        };
        let actual_start = if start < 0 {
            let abs_start = start.saturating_abs() as usize;
            char_count.saturating_sub(abs_start)
        } else {
            start as usize
        };

        // How many chars to take after skipping `actual_start`; `None` =
        // take the rest.
        let take: Option<usize> = match length {
            Some(len) if len < 0 => {
                let abs_end = len.saturating_abs() as usize;
                let end_pos = char_count.saturating_sub(abs_end);
                // `actual_start` is clamped to `char_count` only on the
                // negative-start branch; clamp here so the subtraction
                // can't underflow-skip on a past-the-end positive start.
                Some(end_pos.saturating_sub(actual_start.min(char_count)))
            }
            Some(len) => Some(len as usize),
            None => None,
        };

        let byte_start = char_to_byte_offset(string, actual_start);
        let byte_end = match take {
            Some(n) => byte_start + char_to_byte_offset(&string[byte_start..], n),
            None => string.len(),
        };
        (byte_start, byte_end)
    };

    Ok(&string[byte_start..byte_end])
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
pub(crate) fn in_<'a>(
    cx: &mut Cx<'_, 'a>,
    needle: &'a DataValue<'a>,
    haystack: &'a DataValue<'a>,
) -> Result<bool> {
    Ok(match haystack {
        // String haystack — substring check (needle must be a string).
        // Charged the haystack's bytes, which the search reads.
        DataValue::String(h) => match needle {
            DataValue::String(n) => {
                cx.charge_bytes(h.len())?;
                h.contains(*n)
            }
            _ => false,
        },
        // Array haystack — element-equality check via arena-native
        // strict-equals. Charged its full length up front, the way the
        // iterators charge theirs, so the cost does not depend on where
        // (or whether) the needle is found.
        DataValue::Array(items) => {
            cx.charge(items.len() as u64)?;
            let mut found = false;
            for it in items.iter() {
                // A failed comparison counts as "not this item", except an
                // exhausted budget, which is final.
                match crate::operators::comparison::compare_equals(
                    it, needle, true, cx.engine, cx.ctx,
                ) {
                    Ok(true) => {
                        found = true;
                        break;
                    }
                    Ok(false) => {}
                    Err(e) if matches!(e.kind, crate::ErrorKind::BudgetExceeded { .. }) => {
                        return Err(e);
                    }
                    Err(_) => {}
                }
            }
            found
        }
        _ => false,
    })
}

/// `starts_with(text, prefix)`: whether `text` starts with `prefix`.
#[cfg(feature = "ext-string")]
#[inline]
pub(crate) fn starts_with<'a>(cx: &mut Cx<'_, 'a>, text: &'a str, prefix: &'a str) -> Result<bool> {
    // The test reads at most the needle's length.
    cx.charge_bytes(prefix.len().min(text.len()))?;
    Ok(text.starts_with(prefix))
}

/// `ends_with(text, suffix)`: whether `text` ends with `suffix`.
#[cfg(feature = "ext-string")]
#[inline]
pub(crate) fn ends_with<'a>(cx: &mut Cx<'_, 'a>, text: &'a str, suffix: &'a str) -> Result<bool> {
    // The test reads at most the needle's length.
    cx.charge_bytes(suffix.len().min(text.len()))?;
    Ok(text.ends_with(suffix))
}

/// `upper(text)`: `text` upper-cased.
#[cfg(feature = "ext-string")]
#[inline]
pub(crate) fn upper<'a>(cx: &mut Cx<'_, 'a>, s: &'a str) -> Result<&'a str> {
    cx.charge_bytes(s.len())?;
    if s.is_ascii() {
        return Ok(ascii_case(s, cx.arena, str::make_ascii_uppercase));
    }
    Ok(cx.arena.alloc_str(&s.to_uppercase()))
}

/// `s` re-cased in place in an arena copy: the common ASCII text, which
/// cases per byte, with no context and no change of length.
#[cfg(feature = "ext-string")]
#[inline]
fn ascii_case<'a>(s: &str, arena: &'a Bump, recase: fn(&mut str)) -> &'a str {
    let buf = arena.alloc_str(s);
    recase(buf);
    buf
}

/// `lower(text)`: `text` lower-cased.
#[cfg(feature = "ext-string")]
#[inline]
pub(crate) fn lower<'a>(cx: &mut Cx<'_, 'a>, s: &'a str) -> Result<&'a str> {
    cx.charge_bytes(s.len())?;
    if s.is_ascii() {
        return Ok(ascii_case(s, cx.arena, str::make_ascii_lowercase));
    }
    // `str::to_lowercase`, not a per-character mapping: lower-casing has
    // context rules a character alone cannot apply (a Greek capital sigma
    // ends a word as `ς`, so "ΟΔΟΣ" is "οδος", not "οδοσ").
    Ok(cx.arena.alloc_str(&s.to_lowercase()))
}

/// `trim(text)`: `text` without leading and trailing whitespace.
#[cfg(feature = "ext-string")]
#[inline]
pub(crate) fn trim<'a>(cx: &mut Cx<'_, 'a>, s: &'a str) -> Result<&'a str> {
    // An all-whitespace string is scanned end to end.
    cx.charge_bytes(s.len())?;
    // `s` is already arena-resident, so the trimmed view is an arena
    // sub-slice; no re-copy needed.
    Ok(s.trim())
}

/// `split(text, delimiter)`: the parts of `text` between occurrences of
/// a plain-string delimiter, built directly in the arena. An empty
/// delimiter splits into characters.
#[cfg(feature = "ext-string")]
#[inline]
pub(crate) fn split<'a>(
    cx: &mut Cx<'_, 'a>,
    text: &'a str,
    delim: &'a str,
) -> Result<&'a DataValue<'a>> {
    // The text's bytes now, and one per part produced in
    // `split_arena_normal`.
    cx.charge_bytes(text.len())?;
    split_arena_normal(text, delim, cx.ctx, cx.arena)
}

#[cfg(feature = "ext-string")]
#[inline]
fn split_arena_normal<'a>(
    text: &str,
    delim: &str,
    ctx: &mut ContextStack<'_>,
    arena: &'a Bump,
) -> Result<&'a DataValue<'a>> {
    if text.is_empty() {
        // Empty input → [""].
        let item: &'a str = "";
        let slice = bumpalo::vec![in arena; DataValue::String(item)].into_bump_slice();
        return Ok(arena.alloc(DataValue::Array(slice)));
    }
    if delim.is_empty() {
        // Empty delimiter → split into individual characters.
        let parts = text.chars().count();
        ctx.charge(parts as u64)?;
        let mut items: bumpalo::collections::Vec<'a, DataValue<'a>> =
            bumpalo::collections::Vec::with_capacity_in(parts, arena);
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
    // Charged once the parts exist, to avoid a second scan to count them.
    // Safe to charge after: there are at most as many parts as the text
    // has bytes plus one, and the text was charged before the split.
    ctx.charge(items.len() as u64)?;
    Ok(arena.alloc(DataValue::Array(items.into_bump_slice())))
}
