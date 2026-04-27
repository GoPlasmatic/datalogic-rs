use crate::{CompiledNode, DataLogic, Result};
// =============================================================================
// Arena-mode string operators
// =============================================================================
//
// Pre-evaluate args via `evaluate_node` (so var lookups borrow), then
// build the result. For string-producing ops, the result is allocated as
// `&'a str` in the arena via `arena.alloc_str` — no heap `String`.

use crate::arena::{DataContextStack, DataValue, to_string_arena};
use bumpalo::Bump;

#[inline]
pub(crate) fn evaluate_cat_arena<'a>(
    args: &'a [CompiledNode],
    actx: &mut DataContextStack<'a>,
    engine: &DataLogic,
    arena: &'a Bump,
) -> Result<&'a DataValue<'a>> {
    // Build the concatenated string using a bumpalo String to avoid heap alloc.
    let mut buf = bumpalo::collections::String::new_in(arena);
    for arg in args {
        let av = engine.evaluate_node(arg, actx, arena)?;
        match av {
            // For arrays, concat each item's string form.
            DataValue::Array(items) => {
                for it in *items {
                    buf.push_str(to_string_arena(it, arena));
                }
            }
            _ => buf.push_str(to_string_arena(av, arena)),
        }
    }
    Ok(arena.alloc(DataValue::String(buf.into_bump_str())))
}

/// `substr` — char-indexed substring extraction. Negative start counts from
/// end; negative length is treated as an end position.
#[inline]
pub(crate) fn evaluate_substr_arena<'a>(
    args: &'a [CompiledNode],
    actx: &mut DataContextStack<'a>,
    engine: &DataLogic,
    arena: &'a Bump,
) -> Result<&'a DataValue<'a>> {
    if args.is_empty() {
        return Ok(crate::arena::pool::singleton_empty_string());
    }

    let s_av = engine.evaluate_node(&args[0], actx, arena)?;
    let string = to_string_arena(s_av, arena);
    let char_count = string.chars().count();

    // Read start (defaults to 0). Literal fast path skips dispatch.
    let start: i64 = if args.len() > 1 {
        if let CompiledNode::Value { value, .. } = &args[1] {
            value.as_i64().unwrap_or(0)
        } else {
            engine
                .evaluate_node(&args[1], actx, arena)?
                .as_i64()
                .unwrap_or(0)
        }
    } else {
        0
    };

    // Read optional length.
    let length: Option<i64> = if args.len() > 2 {
        if let CompiledNode::Value { value, .. } = &args[2] {
            value.as_i64()
        } else {
            engine.evaluate_node(&args[2], actx, arena)?.as_i64()
        }
    } else {
        None
    };

    let actual_start = if start < 0 {
        let abs_start = start.saturating_abs() as usize;
        char_count.saturating_sub(abs_start)
    } else {
        (start as usize).min(char_count)
    };

    let result_str: String = if let Some(len) = length {
        if len < 0 {
            // Negative length = end position from end of string.
            let abs_end = len.saturating_abs() as usize;
            let end_pos = char_count.saturating_sub(abs_end);
            if end_pos > actual_start {
                string
                    .chars()
                    .skip(actual_start)
                    .take(end_pos - actual_start)
                    .collect()
            } else {
                String::new()
            }
        } else if len == 0 {
            String::new()
        } else {
            let take_count = (len as usize).min(char_count.saturating_sub(actual_start));
            string.chars().skip(actual_start).take(take_count).collect()
        }
    } else {
        string.chars().skip(actual_start).collect()
    };

    if result_str.is_empty() {
        return Ok(crate::arena::pool::singleton_empty_string());
    }
    Ok(arena.alloc(DataValue::String(arena.alloc_str(&result_str))))
}

/// Native arena-mode `in` — checks whether a needle is contained in a
/// haystack (string-substring, array-element).
#[inline]
pub(crate) fn evaluate_in_arena<'a>(
    args: &'a [CompiledNode],
    actx: &mut DataContextStack<'a>,
    engine: &DataLogic,
    arena: &'a Bump,
) -> Result<&'a DataValue<'a>> {
    if args.len() < 2 {
        return Ok(crate::arena::pool::singleton_false());
    }
    let needle = engine.evaluate_node(&args[0], actx, arena)?;
    let haystack = engine.evaluate_node(&args[1], actx, arena)?;

    let result = match haystack {
        // String haystack — substring check (needle must be a string).
        DataValue::String(h) => match needle {
            DataValue::String(n) => h.contains(*n),
            _ => false,
        },
        // Array haystack — element-equality check via arena-native
        // strict-equals.
        DataValue::Array(items) => items.iter().any(|it| {
            crate::operators::comparison::compare_equals_arena(it, needle, true, engine)
                .unwrap_or(false)
        }),
        _ => false,
    };
    Ok(crate::arena::pool::singleton_bool(result))
}

#[cfg(feature = "ext-string")]
#[inline]
pub(crate) fn evaluate_starts_with_arena<'a>(
    args: &'a [CompiledNode],
    actx: &mut DataContextStack<'a>,
    engine: &DataLogic,
    arena: &'a Bump,
) -> Result<&'a DataValue<'a>> {
    if args.len() < 2 {
        return Err(crate::constants::invalid_args());
    }
    let s = engine.evaluate_node(&args[0], actx, arena)?;
    let p = engine.evaluate_node(&args[1], actx, arena)?;
    let s_str = to_string_arena(s, arena);
    let p_str = to_string_arena(p, arena);
    Ok(crate::arena::pool::singleton_bool(s_str.starts_with(p_str)))
}

#[cfg(feature = "ext-string")]
#[inline]
pub(crate) fn evaluate_ends_with_arena<'a>(
    args: &'a [CompiledNode],
    actx: &mut DataContextStack<'a>,
    engine: &DataLogic,
    arena: &'a Bump,
) -> Result<&'a DataValue<'a>> {
    if args.len() < 2 {
        return Err(crate::constants::invalid_args());
    }
    let s = engine.evaluate_node(&args[0], actx, arena)?;
    let p = engine.evaluate_node(&args[1], actx, arena)?;
    let s_str = to_string_arena(s, arena);
    let p_str = to_string_arena(p, arena);
    Ok(crate::arena::pool::singleton_bool(s_str.ends_with(p_str)))
}

#[cfg(feature = "ext-string")]
#[inline]
pub(crate) fn evaluate_upper_arena<'a>(
    args: &'a [CompiledNode],
    actx: &mut DataContextStack<'a>,
    engine: &DataLogic,
    arena: &'a Bump,
) -> Result<&'a DataValue<'a>> {
    if args.is_empty() {
        return Err(crate::constants::invalid_args());
    }
    let av = engine.evaluate_node(&args[0], actx, arena)?;
    let s = to_string_arena(av, arena);
    Ok(arena.alloc(DataValue::String(arena.alloc_str(&s.to_uppercase()))))
}

#[cfg(feature = "ext-string")]
#[inline]
pub(crate) fn evaluate_lower_arena<'a>(
    args: &'a [CompiledNode],
    actx: &mut DataContextStack<'a>,
    engine: &DataLogic,
    arena: &'a Bump,
) -> Result<&'a DataValue<'a>> {
    if args.is_empty() {
        return Err(crate::constants::invalid_args());
    }
    let av = engine.evaluate_node(&args[0], actx, arena)?;
    let s = to_string_arena(av, arena);
    Ok(arena.alloc(DataValue::String(arena.alloc_str(&s.to_lowercase()))))
}

#[cfg(feature = "ext-string")]
#[inline]
pub(crate) fn evaluate_trim_arena<'a>(
    args: &'a [CompiledNode],
    actx: &mut DataContextStack<'a>,
    engine: &DataLogic,
    arena: &'a Bump,
) -> Result<&'a DataValue<'a>> {
    if args.is_empty() {
        return Err(crate::constants::invalid_args());
    }
    let av = engine.evaluate_node(&args[0], actx, arena)?;
    let s = to_string_arena(av, arena);
    Ok(arena.alloc(DataValue::String(arena.alloc_str(s.trim()))))
}

/// Native arena-mode `split`. Splits text by a plain string delimiter,
/// building the result directly in the arena.
#[cfg(feature = "ext-string")]
#[inline]
pub(crate) fn evaluate_split_arena<'a>(
    args: &'a [CompiledNode],
    actx: &mut DataContextStack<'a>,
    engine: &DataLogic,
    arena: &'a Bump,
) -> Result<&'a DataValue<'a>> {
    if args.len() < 2 {
        return Err(crate::constants::invalid_args());
    }
    let text_av = engine.evaluate_node(&args[0], actx, arena)?;
    let text_str: &'a str = match text_av {
        DataValue::String(s) => s,
        _ => to_string_arena(text_av, arena),
    };

    // Resolve the delimiter as a string. Literal-string fast path skips dispatch.
    let delim_str: &'a str = if let CompiledNode::Value {
        value: datavalue::OwnedDataValue::String(s),
        ..
    } = &args[1]
    {
        arena.alloc_str(s)
    } else {
        let av = engine.evaluate_node(&args[1], actx, arena)?;
        match av {
            DataValue::String(s) => s,
            _ => to_string_arena(av, arena),
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
