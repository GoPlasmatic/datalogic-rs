//! `slice` — array and string slicing with optional start/end/step.

use crate::arena::{ContextStack, DataValue, bvec};
use crate::operators::string::char_to_byte_offset;
use crate::{CompiledNode, Engine, Error, Result};
use bumpalo::Bump;

/// Native arena-mode `slice`. Returns array slices as views over arena items;
/// string slices are allocated in the arena.
#[inline]
pub(crate) fn evaluate_slice<'a>(
    args: &'a [CompiledNode],
    ctx: &mut ContextStack<'a>,
    engine: &Engine,
    arena: &'a Bump,
) -> Result<&'a DataValue<'a>> {
    if args.is_empty() {
        return Err(crate::Error::invalid_args());
    }

    let coll_av = engine.dispatch_node(&args[0], ctx, arena)?;

    // Null passthrough.
    if matches!(coll_av, DataValue::Null) {
        return Ok(crate::arena::singletons::singleton_null());
    }

    // Resolve start/end/step.
    let start = if args.len() > 1 {
        extract_opt_i64_arena(&args[1], ctx, engine, arena)?
    } else {
        None
    };
    let end = if args.len() > 2 {
        extract_opt_i64_arena(&args[2], ctx, engine, arena)?
    } else {
        None
    };
    let step = if args.len() > 3 {
        let s = extract_opt_i64_arena(&args[3], ctx, engine, arena)?.unwrap_or(1);
        if s == 0 {
            return Err(crate::Error::invalid_args());
        }
        s
    } else {
        1
    };

    if let DataValue::Array(items) = coll_av {
        return Ok(slice_array(items, start, end, step, arena));
    }
    if let DataValue::String(s) = coll_av {
        return Ok(slice_string(s, start, end, step, arena));
    }
    Err(crate::Error::invalid_args())
}

/// Composite arena array — slice through the arena items.
#[inline]
fn slice_array<'a>(
    items: &'a [DataValue<'a>],
    start: Option<i64>,
    end: Option<i64>,
    step: i64,
    arena: &'a Bump,
) -> &'a DataValue<'a> {
    let len = items.len() as i64;

    // Contiguous step==1 (the default, no explicit step): the selected
    // indices form the run [s, e), so return a borrowed arena sub-slice
    // directly instead of materializing an index Vec and copying elements.
    if step == 1 {
        let s = normalize_index(start.unwrap_or(0), len);
        let e = normalize_index(end.unwrap_or(len), len);
        if s >= e {
            return crate::arena::singletons::singleton_empty_array();
        }
        return arena.alloc(DataValue::Array(&items[s as usize..e as usize]));
    }

    let indices = slice_indices(len, start, end, step);
    if indices.is_empty() {
        return crate::arena::singletons::singleton_empty_array();
    }
    let mut out = bvec::<DataValue<'a>>(arena, indices.len());
    for i in indices {
        out.push(items[i as usize]);
    }
    arena.alloc(DataValue::Array(out.into_bump_slice()))
}

/// String slice — char-indexed; the contiguous `step == 1` case borrows a
/// sub-slice of the source, other steps allocate the result in the arena.
#[inline]
fn slice_string<'a>(
    s: &'a str,
    start: Option<i64>,
    end: Option<i64>,
    step: i64,
    arena: &'a Bump,
) -> &'a DataValue<'a> {
    // Contiguous step==1: mirror the array arm. The selected chars form the
    // run [a, b), and `s` is already an arena `&'a str`, so return a borrowed
    // sub-slice at the char boundaries instead of collecting chars.
    if step == 1 {
        let char_len = s.chars().count() as i64;
        let a = normalize_index(start.unwrap_or(0), char_len);
        let b = normalize_index(end.unwrap_or(char_len), char_len);
        if a >= b {
            return crate::arena::singletons::singleton_empty_string();
        }
        let byte_a = char_to_byte_offset(s, a as usize);
        let byte_b = byte_a + char_to_byte_offset(&s[byte_a..], (b - a) as usize);
        return arena.alloc(DataValue::String(&s[byte_a..byte_b]));
    }

    let chars: Vec<char> = s.chars().collect();
    let indices = slice_indices(chars.len() as i64, start, end, step);
    let result_string: String = indices.iter().map(|&i| chars[i as usize]).collect();
    let out: &'a str = arena.alloc_str(&result_string);
    arena.alloc(DataValue::String(out))
}

#[inline]
fn extract_opt_i64_arena<'a>(
    node: &'a CompiledNode,
    ctx: &mut ContextStack<'a>,
    engine: &Engine,
    arena: &'a Bump,
) -> Result<Option<i64>> {
    if let CompiledNode::Value { value, .. } = node {
        return match value {
            datavalue::OwnedDataValue::Number(n) => Ok(n.as_i64()),
            datavalue::OwnedDataValue::Null => Ok(None),
            _ => Err(Error::invalid_arguments("NaN")),
        };
    }
    let av = engine.dispatch_node(node, ctx, arena)?;
    match av {
        DataValue::Null => Ok(None),
        _ => match av.as_i64() {
            Some(i) => Ok(Some(i)),
            None => Err(Error::invalid_arguments("NaN")),
        },
    }
}

/// Index list for a slice given start/end/step. Computes the index sequence
/// without materializing values.
#[inline]
fn slice_indices(len: i64, start: Option<i64>, end: Option<i64>, step: i64) -> Vec<i64> {
    let mut out = Vec::new();
    let (actual_start, actual_end) = if step > 0 {
        (
            normalize_index(start.unwrap_or(0), len),
            normalize_index(end.unwrap_or(len), len),
        )
    } else {
        let default_start = len.saturating_sub(1);
        let s = normalize_index(start.unwrap_or(default_start), len);
        let e = if let Some(e) = end {
            if e < -len {
                -1
            } else {
                normalize_index(e, len)
            }
        } else {
            -1
        };
        (s, e)
    };

    let mut i = actual_start;
    while (step > 0 && i < actual_end) || (step < 0 && i > actual_end) {
        if i >= 0 && i < len {
            out.push(i);
        }
        i += step;
    }
    out
}

/// Normalize slice indices with overflow protection.
#[inline]
fn normalize_index(index: i64, len: i64) -> i64 {
    if index < 0 {
        // Use saturating_add to prevent overflow when index is very negative
        let adjusted = len.saturating_add(index);
        adjusted.max(0)
    } else {
        index.min(len)
    }
}
