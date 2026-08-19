//! `group_by` — collapse an array into `{key, items}` rows on a computed key.

use crate::arena::{ContextStack, DataValue, IterGuard, bvec};
use crate::operators::comparison::compare_equals;
use crate::{CompiledNode, Engine, Result};
use bumpalo::Bump;

use super::helpers::{IterArgKind, ResolvedInput, resolve_iter_input};

/// `group_by: [array, key_expr]` → array of `{key, items}` objects.
///
/// The key expression runs once per element under an indexed iter frame
/// (same scoping as `sort`'s extractor: `{"var": ...}` reads the element,
/// `{"val": [[1], ...]}` reads outward). Groups keep the order of first
/// key occurrence, so the output is deterministic for a given input. Keys
/// are kept as evaluated — not stringified — and matched by strict deep
/// equality, so number/bool/null/object keys all group correctly.
///
/// Group lookup is a linear scan with `compare_equals` — O(n·g) for g
/// distinct keys, which is fine for realistic group counts. If profiles
/// ever show a hot string-keyed case, an opportunistic `&str → index` map
/// (falling back to the scan on the first non-string key) is the upgrade
/// path; `DataValue` has no `Hash`/`Ord`, so the scan is the honest
/// general mechanism.
#[inline]
pub(crate) fn evaluate_group_by<'a>(
    args: &'a [CompiledNode],
    iter_arg_kind: IterArgKind,
    ctx: &mut ContextStack<'a>,
    engine: &Engine,
    arena: &'a Bump,
) -> Result<&'a DataValue<'a>> {
    // The key expression is mandatory — a keyless group_by has no meaning.
    if args.len() < 2 {
        return Err(crate::Error::invalid_args());
    }

    let src = match resolve_iter_input(&args[0], iter_arg_kind, ctx, engine, arena)? {
        ResolvedInput::Iterable(s) => s,
        ResolvedInput::Empty => return Ok(crate::arena::singletons::singleton_empty_array()),
        ResolvedInput::Bridge(av) => {
            // Bridge is never Array/Null (see ResolvedInput::Bridge), so a
            // group_by input reaching here is a scalar or object: not groupable.
            debug_assert!(!matches!(av, DataValue::Array(_) | DataValue::Null));
            return Err(crate::Error::invalid_args());
        }
    };

    let len = src.len();
    if len == 0 {
        return Ok(crate::arena::singletons::singleton_empty_array());
    }

    let key_expr = &args[1];

    // Accumulate (key, items) in first-occurrence order.
    let mut groups = bvec::<(
        &'a DataValue<'a>,
        bumpalo::collections::Vec<'a, DataValue<'a>>,
    )>(arena, len.min(8));
    let mut guard = IterGuard::new(ctx);
    for i in 0..len {
        let item = src.get(i);
        guard.step_indexed(item, i);
        let key = engine.dispatch_node(key_expr, guard.stack(), arena)?;

        let mut matched = false;
        for (k, items) in groups.iter_mut() {
            if compare_equals(k, key, true, engine)? {
                items.push(*item);
                matched = true;
                break;
            }
        }
        if !matched {
            let mut items = bvec::<DataValue<'a>>(arena, 4);
            items.push(*item);
            groups.push((key, items));
        }
    }
    drop(guard);

    let rows = arena.alloc_slice_fill_iter(groups.into_iter().map(|(key, items)| {
        let pairs = arena.alloc([
            ("key", *key),
            ("items", DataValue::Array(items.into_bump_slice())),
        ]);
        DataValue::Object(&pairs[..])
    }));
    Ok(arena.alloc(DataValue::Array(rows)))
}
