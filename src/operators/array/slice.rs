//! `slice` — array and string slicing with optional start/end/step.

use crate::arena::{DataContextStack, DataValue, bvec};
use crate::{CompiledNode, DataLogic, Error, Result};
use bumpalo::Bump;

/// Native arena-mode `slice`. Returns array slices as views over arena items;
/// string slices are allocated in the arena.
#[inline]
pub(crate) fn evaluate_slice_arena<'a>(
    args: &'a [CompiledNode],
    actx: &mut DataContextStack<'a>,
    engine: &DataLogic,
    arena: &'a Bump,
) -> Result<&'a DataValue<'a>> {
    if args.is_empty() {
        return Err(crate::constants::invalid_args());
    }

    let coll_av = engine.evaluate_node(&args[0], actx, arena)?;

    // Null passthrough.
    if matches!(coll_av, DataValue::Null) {
        return Ok(crate::arena::pool::singleton_null());
    }

    // Resolve start/end/step.
    let start = if args.len() > 1 {
        extract_opt_i64_arena(&args[1], actx, engine, arena)?
    } else {
        None
    };
    let end = if args.len() > 2 {
        extract_opt_i64_arena(&args[2], actx, engine, arena)?
    } else {
        None
    };
    let step = if args.len() > 3 {
        let s = extract_opt_i64_arena(&args[3], actx, engine, arena)?.unwrap_or(1);
        if s == 0 {
            return Err(crate::constants::invalid_args());
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
    Err(crate::constants::invalid_args())
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
    let indices = slice_indices(len, start, end, step);
    if indices.is_empty() {
        return crate::arena::pool::singleton_empty_array();
    }
    let mut out = bvec::<DataValue<'a>>(arena, indices.len());
    for i in indices {
        out.push(crate::arena::value::reborrow_arena_value(
            &items[i as usize],
        ));
    }
    arena.alloc(DataValue::Array(out.into_bump_slice()))
}

/// String slice — allocate result in the arena.
#[inline]
fn slice_string<'a>(
    s: &str,
    start: Option<i64>,
    end: Option<i64>,
    step: i64,
    arena: &'a Bump,
) -> &'a DataValue<'a> {
    let chars: Vec<char> = s.chars().collect();
    let result_string = slice_chars(&chars, chars.len() as i64, start, end, step);
    let s_arena: &'a str = arena.alloc_str(&result_string);
    arena.alloc(DataValue::String(s_arena))
}

#[inline]
fn extract_opt_i64_arena<'a>(
    node: &'a CompiledNode,
    actx: &mut DataContextStack<'a>,
    engine: &DataLogic,
    arena: &'a Bump,
) -> Result<Option<i64>> {
    if let CompiledNode::Value { value, .. } = node {
        return match value {
            datavalue::OwnedDataValue::Number(n) => Ok(n.as_i64()),
            datavalue::OwnedDataValue::Null => Ok(None),
            _ => Err(Error::InvalidArguments("NaN".to_string())),
        };
    }
    let av = engine.evaluate_node(node, actx, arena)?;
    match av {
        DataValue::Null => Ok(None),
        _ => match av.as_i64() {
            Some(i) => Ok(Some(i)),
            None => Err(Error::InvalidArguments("NaN".to_string())),
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

#[inline]
fn slice_chars(
    chars: &[char],
    len: i64,
    start: Option<i64>,
    end: Option<i64>,
    step: i64,
) -> String {
    let mut result = String::new();

    let (actual_start, actual_end) = if step > 0 {
        let s = normalize_index(start.unwrap_or(0), len);
        let e = normalize_index(end.unwrap_or(len), len);
        (s, e)
    } else {
        let default_start = len.saturating_sub(1);
        let s = normalize_index(start.unwrap_or(default_start), len);
        let e = if let Some(e) = end {
            normalize_index(e, len)
        } else {
            -1
        };
        (s, e)
    };

    if step > 0 {
        let mut i = actual_start;
        while i < actual_end && i < len {
            if i >= 0 && (i as usize) < chars.len() {
                result.push(chars[i as usize]);
            }
            i = i.saturating_add(step);
            if step > 0 && i < actual_start {
                break;
            }
        }
    } else {
        let mut i = actual_start;
        while i > actual_end && i >= 0 && i < len {
            if (i as usize) < chars.len() {
                result.push(chars[i as usize]);
            }
            let next_i = i.saturating_add(step);
            if step < 0 && next_i > i {
                break;
            }
            i = next_i;
        }
    }

    result
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
