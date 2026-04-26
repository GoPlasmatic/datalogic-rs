#[cfg(feature = "ext-string")]
use regex::Regex;
use serde_json::Value;

#[cfg(feature = "ext-string")]
use crate::constants::INVALID_ARGS;
#[cfg(feature = "ext-string")]
use crate::error::Error;
use crate::{CompiledNode, DataLogic, Result};
// =============================================================================
// Arena-mode string operators
// =============================================================================
//
// Pre-evaluate args via `evaluate_arena_node` (so var lookups borrow), then
// build the result. For string-producing ops, the result is allocated as
// `&'a str` in the arena via `arena.alloc_str` — no heap `String`.

use crate::arena::{ArenaContextStack, ArenaValue, arena_to_value_cow, to_string_arena};
use bumpalo::Bump;

#[inline]
pub(crate) fn evaluate_cat_arena<'a>(
    args: &[CompiledNode],
    actx: &mut ArenaContextStack<'a>,
    engine: &DataLogic,
    arena: &'a Bump,
) -> Result<&'a ArenaValue<'a>> {
    // Build the concatenated string using a bumpalo String to avoid heap alloc.
    let mut buf = bumpalo::collections::String::new_in(arena);
    for arg in args {
        let av = engine.evaluate_arena_node(arg, actx, arena)?;
        match av {
            // For arrays, concat each item's string form.
            ArenaValue::Array(items) => {
                for it in *items {
                    buf.push_str(to_string_arena(it, arena));
                }
            }
            ArenaValue::InputRef(Value::Array(arr)) => {
                for it in arr {
                    let part = match it {
                        Value::String(s) => s.as_str(),
                        Value::Null => "",
                        _ => arena.alloc_str(&it.to_string()),
                    };
                    buf.push_str(part);
                }
            }
            _ => buf.push_str(to_string_arena(av, arena)),
        }
    }
    Ok(arena.alloc(ArenaValue::String(buf.into_bump_str())))
}

/// Native arena-mode substr — char-indexed substring extraction. Mirrors
/// the value-mode `evaluate_substr` semantics (negative start counts from
/// end; negative length is treated as an end position).
#[inline]
pub(crate) fn evaluate_substr_arena<'a>(
    args: &[CompiledNode],
    actx: &mut ArenaContextStack<'a>,
    engine: &DataLogic,
    arena: &'a Bump,
) -> Result<&'a ArenaValue<'a>> {
    if args.is_empty() {
        return Ok(crate::arena::pool::singleton_empty_string());
    }

    let s_av = engine.evaluate_arena_node(&args[0], actx, arena)?;
    let string = to_string_arena(s_av, arena);
    let char_count = string.chars().count();

    // Read start (defaults to 0). Literal fast path skips dispatch.
    let start: i64 = if args.len() > 1 {
        if let CompiledNode::Value { value, .. } = &args[1] {
            value.as_i64().unwrap_or(0)
        } else {
            engine
                .evaluate_arena_node(&args[1], actx, arena)?
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
            engine.evaluate_arena_node(&args[2], actx, arena)?.as_i64()
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
    Ok(arena.alloc(ArenaValue::String(arena.alloc_str(&result_str))))
}

/// Native arena-mode `in` — checks whether a needle is contained in a
/// haystack (string-substring, array-element).
#[inline]
pub(crate) fn evaluate_in_arena<'a>(
    args: &[CompiledNode],
    actx: &mut ArenaContextStack<'a>,
    engine: &DataLogic,
    arena: &'a Bump,
) -> Result<&'a ArenaValue<'a>> {
    if args.len() < 2 {
        return Ok(crate::arena::pool::singleton_false());
    }
    let needle = engine.evaluate_arena_node(&args[0], actx, arena)?;
    let haystack = engine.evaluate_arena_node(&args[1], actx, arena)?;

    let result = match haystack {
        // String haystack — substring check (needle must be a string).
        ArenaValue::String(h) => match needle {
            ArenaValue::String(n) => h.contains(*n),
            ArenaValue::InputRef(Value::String(n)) => h.contains(n.as_str()),
            _ => false,
        },
        ArenaValue::InputRef(Value::String(h)) => match needle {
            ArenaValue::String(n) => h.contains(*n),
            ArenaValue::InputRef(Value::String(n)) => h.contains(n.as_str()),
            _ => false,
        },
        // Array haystack — element-equality check using arena_to_value_cow
        // so InputRefs and arena-resident values compare consistently.
        ArenaValue::Array(items) => {
            let needle_cow = arena_to_value_cow(needle);
            items.iter().any(|it| {
                let it_cow = arena_to_value_cow(it);
                it_cow.as_ref() == needle_cow.as_ref()
            })
        }
        ArenaValue::InputRef(Value::Array(arr)) => {
            let needle_cow = arena_to_value_cow(needle);
            arr.iter().any(|v| v == needle_cow.as_ref())
        }
        _ => false,
    };
    Ok(crate::arena::pool::singleton_bool(result))
}

#[cfg(feature = "ext-string")]
#[inline]
pub(crate) fn evaluate_starts_with_arena<'a>(
    args: &[CompiledNode],
    actx: &mut ArenaContextStack<'a>,
    engine: &DataLogic,
    arena: &'a Bump,
) -> Result<&'a ArenaValue<'a>> {
    if args.len() < 2 {
        return Err(Error::InvalidArguments(INVALID_ARGS.into()));
    }
    let s = engine.evaluate_arena_node(&args[0], actx, arena)?;
    let p = engine.evaluate_arena_node(&args[1], actx, arena)?;
    let s_str = to_string_arena(s, arena);
    let p_str = to_string_arena(p, arena);
    Ok(crate::arena::pool::singleton_bool(s_str.starts_with(p_str)))
}

#[cfg(feature = "ext-string")]
#[inline]
pub(crate) fn evaluate_ends_with_arena<'a>(
    args: &[CompiledNode],
    actx: &mut ArenaContextStack<'a>,
    engine: &DataLogic,
    arena: &'a Bump,
) -> Result<&'a ArenaValue<'a>> {
    if args.len() < 2 {
        return Err(Error::InvalidArguments(INVALID_ARGS.into()));
    }
    let s = engine.evaluate_arena_node(&args[0], actx, arena)?;
    let p = engine.evaluate_arena_node(&args[1], actx, arena)?;
    let s_str = to_string_arena(s, arena);
    let p_str = to_string_arena(p, arena);
    Ok(crate::arena::pool::singleton_bool(s_str.ends_with(p_str)))
}

#[cfg(feature = "ext-string")]
#[inline]
pub(crate) fn evaluate_upper_arena<'a>(
    args: &[CompiledNode],
    actx: &mut ArenaContextStack<'a>,
    engine: &DataLogic,
    arena: &'a Bump,
) -> Result<&'a ArenaValue<'a>> {
    if args.is_empty() {
        return Err(Error::InvalidArguments(INVALID_ARGS.into()));
    }
    let av = engine.evaluate_arena_node(&args[0], actx, arena)?;
    let s = to_string_arena(av, arena);
    Ok(arena.alloc(ArenaValue::String(arena.alloc_str(&s.to_uppercase()))))
}

#[cfg(feature = "ext-string")]
#[inline]
pub(crate) fn evaluate_lower_arena<'a>(
    args: &[CompiledNode],
    actx: &mut ArenaContextStack<'a>,
    engine: &DataLogic,
    arena: &'a Bump,
) -> Result<&'a ArenaValue<'a>> {
    if args.is_empty() {
        return Err(Error::InvalidArguments(INVALID_ARGS.into()));
    }
    let av = engine.evaluate_arena_node(&args[0], actx, arena)?;
    let s = to_string_arena(av, arena);
    Ok(arena.alloc(ArenaValue::String(arena.alloc_str(&s.to_lowercase()))))
}

#[cfg(feature = "ext-string")]
#[inline]
pub(crate) fn evaluate_trim_arena<'a>(
    args: &[CompiledNode],
    actx: &mut ArenaContextStack<'a>,
    engine: &DataLogic,
    arena: &'a Bump,
) -> Result<&'a ArenaValue<'a>> {
    if args.is_empty() {
        return Err(Error::InvalidArguments(INVALID_ARGS.into()));
    }
    let av = engine.evaluate_arena_node(&args[0], actx, arena)?;
    let s = to_string_arena(av, arena);
    Ok(arena.alloc(ArenaValue::String(arena.alloc_str(s.trim()))))
}

/// Native arena-mode `split`. Plain-string delimiter and named-capture
/// regex paths build their result directly in the arena.
#[cfg(feature = "ext-string")]
#[inline]
pub(crate) fn evaluate_split_arena<'a>(
    args: &[CompiledNode],
    actx: &mut ArenaContextStack<'a>,
    engine: &DataLogic,
    arena: &'a Bump,
) -> Result<&'a ArenaValue<'a>> {
    if args.len() < 2 {
        return Err(Error::InvalidArguments(INVALID_ARGS.into()));
    }
    let text_av = engine.evaluate_arena_node(&args[0], actx, arena)?;
    let text_str: &'a str = match text_av {
        ArenaValue::String(s) => s,
        ArenaValue::InputRef(Value::String(s)) => arena.alloc_str(s.as_str()),
        _ => to_string_arena(text_av, arena),
    };

    // Resolve the delimiter as a string. Literal-string fast path skips dispatch.
    let delim_str: &'a str = if let CompiledNode::Value {
        value: Value::String(s),
        ..
    } = &args[1]
    {
        arena.alloc_str(s)
    } else {
        let av = engine.evaluate_arena_node(&args[1], actx, arena)?;
        match av {
            ArenaValue::String(s) => s,
            ArenaValue::InputRef(Value::String(s)) => arena.alloc_str(s.as_str()),
            _ => to_string_arena(av, arena),
        }
    };

    // Named-capture regex path (mirrors value-mode behavior — dynamic
    // regex delimiter with `(?P<…>)` named groups returns an object).
    if delim_str.contains("(?P<")
        && let Ok(re) = Regex::new(delim_str)
    {
        let capture_names: Vec<_> = re.capture_names().flatten().collect();
        if !capture_names.is_empty() {
            return if let Some(captures) = re.captures(text_str) {
                let mut pairs: bumpalo::collections::Vec<'a, (&'a str, ArenaValue<'a>)> =
                    bumpalo::collections::Vec::with_capacity_in(capture_names.len(), arena);
                for name in capture_names {
                    if let Some(m) = captures.name(name) {
                        let k: &'a str = arena.alloc_str(name);
                        let v: &'a str = arena.alloc_str(m.as_str());
                        pairs.push((k, ArenaValue::String(v)));
                    }
                }
                Ok(arena.alloc(ArenaValue::Object(pairs.into_bump_slice())))
            } else {
                Ok(arena.alloc(ArenaValue::Object(&[])))
            };
        }
    }

    // Plain string split path.
    split_arena_normal(text_str, delim_str, arena)
}

#[cfg(feature = "ext-string")]
#[inline]
fn split_arena_normal<'a>(text: &str, delim: &str, arena: &'a Bump) -> Result<&'a ArenaValue<'a>> {
    if text.is_empty() {
        // Mirrors value-mode: empty input → ["" ].
        let item: &'a str = "";
        let slice = bumpalo::vec![in arena; ArenaValue::String(item)].into_bump_slice();
        return Ok(arena.alloc(ArenaValue::Array(slice)));
    }
    if delim.is_empty() {
        // Empty delimiter → split into individual characters.
        let mut items: bumpalo::collections::Vec<'a, ArenaValue<'a>> =
            bumpalo::collections::Vec::with_capacity_in(text.chars().count(), arena);
        for c in text.chars() {
            // Per-char arena string. For ASCII, a 1-byte alloc per char.
            let mut buf = bumpalo::collections::String::new_in(arena);
            buf.push(c);
            items.push(ArenaValue::String(buf.into_bump_str()));
        }
        return Ok(arena.alloc(ArenaValue::Array(items.into_bump_slice())));
    }
    let mut items: bumpalo::collections::Vec<'a, ArenaValue<'a>> =
        bumpalo::collections::Vec::new_in(arena);
    for part in text.split(delim) {
        items.push(ArenaValue::String(arena.alloc_str(part)));
    }
    Ok(arena.alloc(ArenaValue::Array(items.into_bump_slice())))
}

/// Arena variant of split-with-regex (compiled regex form).
#[cfg(feature = "ext-string")]
#[inline]
pub(crate) fn evaluate_split_with_regex_arena<'a>(
    args: &[CompiledNode],
    actx: &mut ArenaContextStack<'a>,
    engine: &DataLogic,
    regex: &Regex,
    capture_names: &[Box<str>],
    arena: &'a Bump,
) -> Result<&'a ArenaValue<'a>> {
    if args.is_empty() {
        return Err(Error::InvalidArguments(INVALID_ARGS.into()));
    }
    let text_av = engine.evaluate_arena_node(&args[0], actx, arena)?;
    let text_str = match text_av {
        ArenaValue::String(s) => *s,
        ArenaValue::InputRef(Value::String(s)) => s.as_str(),
        _ => to_string_arena(text_av, arena),
    };

    if let Some(captures) = regex.captures(text_str) {
        let mut pairs: bumpalo::collections::Vec<'a, (&'a str, ArenaValue<'a>)> =
            bumpalo::collections::Vec::with_capacity_in(capture_names.len(), arena);
        for name in capture_names {
            if let Some(m) = captures.name(name) {
                let k: &'a str = arena.alloc_str(name);
                let v: &'a str = arena.alloc_str(m.as_str());
                pairs.push((k, ArenaValue::String(v)));
            }
        }
        Ok(arena.alloc(ArenaValue::Object(pairs.into_bump_slice())))
    } else {
        Ok(arena.alloc(ArenaValue::Object(&[])))
    }
}
