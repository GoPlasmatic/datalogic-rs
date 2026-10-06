//! A cap on how deep a `reduce` accumulator may nest.
//!
//! `reduce` is the one operator that feeds its own result back in: each
//! step can wrap the previous accumulator in new arrays or objects
//! (`{"reduce": [xs, [{"var": "accumulator"}], null]}`), so over a long
//! list the value grows one level per item. Everything else is bounded by
//! the rule's own nesting (capped at compile time) and the input's (capped
//! by the parser; tensor rank is capped too). Serialising, comparing and
//! copying a value recurse once per level, so a value a few tens of
//! thousands of levels deep overflows the stack and aborts the process,
//! which no `catch_unwind` can stop. [`AccumulatorDepth`] measures the
//! accumulator as the fold runs and turns that into an error long before.

use datavalue::OwnedDataValue;

use crate::arena::DataValue;
use crate::node::CompiledNode;
use crate::{Error, Result};

/// Deepest a `reduce` accumulator may nest, counting each array or object
/// level. Far above what the parser accepts as input (256) and anything a
/// real fold builds, and far below the depth at which recursing over the
/// value threatens a small (512 KiB – 2 MiB) thread stack.
pub(crate) const MAX_ACCUMULATOR_DEPTH: u32 = 1024;

/// How far past the last measurement the accumulator may grow before the
/// next one: steps between measurements are this over the body's
/// [`growth`] bound, so between two measurements the value stays within
/// `MAX_ACCUMULATOR_DEPTH + OVERSHOOT` levels.
const OVERSHOOT: u32 = 256;

/// Measures successive accumulators of one `reduce`.
///
/// Each step's accumulator is mostly made of the previous one: wrapped
/// whole (`[acc]`, `{"next": acc}`) or with its elements copied into a new
/// list (`merge`). Arena values never move during an evaluation, so a part
/// that is the very slice measured before needs no second look: the last
/// measured accumulator has its bound, and a child at the same position as
/// one of its children, and the same slice, is no deeper than they were.
/// Only the parts built since are walked.
///
/// And the fold need not be measured every step. When the body is plain
/// built-ins, one step deepens the accumulator by at most a bound read off
/// the body ([`growth`]), so measuring every `OVERSHOOT / growth` steps,
/// and once at the end, keeps every accumulator within reach of the cap.
/// A body that calls a custom operator is measured every step.
///
/// The depth kept is an upper bound (a child dropped from the list can
/// leave it higher than the true depth), so it can only err on the side of
/// the cap.
pub(super) struct AccumulatorDepth<'a> {
    body: &'a CompiledNode,
    /// Steps between measurements, worked out on the first container
    /// accumulator so a fold to a number never pays for it.
    stride: Option<u32>,
    /// Container steps since the last measurement.
    since: u32,
    /// The last measured container accumulator and its depth bound.
    prev: Option<(Container<'a>, u32)>,
}

/// A non-empty array or object, by its slice.
#[derive(Clone, Copy)]
enum Container<'a> {
    Array(&'a [DataValue<'a>]),
    Object(&'a [(&'a str, DataValue<'a>)]),
}

impl<'a> Container<'a> {
    #[inline]
    fn of(value: &DataValue<'a>) -> Option<Self> {
        match *value {
            DataValue::Array(items) if !items.is_empty() => Some(Self::Array(items)),
            DataValue::Object(pairs) if !pairs.is_empty() => Some(Self::Object(pairs)),
            _ => None,
        }
    }

    /// The same slice (address and length), not merely equal contents.
    #[inline]
    fn is(&self, value: &DataValue<'a>) -> bool {
        match (*self, *value) {
            (Self::Array(a), DataValue::Array(b)) => std::ptr::eq(a, b),
            (Self::Object(a), DataValue::Object(b)) => std::ptr::eq(a, b),
            _ => false,
        }
    }

    #[inline]
    fn len(&self) -> usize {
        match self {
            Self::Array(items) => items.len(),
            Self::Object(pairs) => pairs.len(),
        }
    }

    #[inline]
    fn child(&self, at: usize) -> &'a DataValue<'a> {
        match *self {
            Self::Array(items) => &items[at],
            Self::Object(pairs) => &pairs[at].1,
        }
    }
}

impl<'a> AccumulatorDepth<'a> {
    pub(super) fn new(body: &'a CompiledNode) -> Self {
        Self {
            body,
            stride: None,
            since: 0,
            prev: None,
        }
    }

    /// After one step of the fold: errors when `acc` is measured and
    /// nests deeper than [`MAX_ACCUMULATOR_DEPTH`].
    #[inline]
    pub(super) fn step(&mut self, acc: &DataValue<'a>) -> Result<()> {
        if Container::of(acc).is_none() {
            // A scalar or an empty container: depth 0 or 1.
            self.prev = None;
            self.since = 0;
            return Ok(());
        }
        self.since += 1;
        let stride = *self.stride.get_or_insert_with(|| match growth(self.body) {
            Some(0) => OVERSHOOT,
            Some(g) => (OVERSHOOT / g).max(1),
            None => 1,
        });
        if self.since >= stride {
            self.measure(acc)?;
        }
        Ok(())
    }

    /// After the last step: measures the result unless the last step did.
    #[inline]
    pub(super) fn finish(&mut self, acc: &DataValue<'a>) -> Result<()> {
        if self.since > 0 {
            self.measure(acc)?;
        }
        Ok(())
    }

    #[cold]
    fn measure(&mut self, acc: &DataValue<'a>) -> Result<()> {
        self.since = 0;
        let Some(top) = Container::of(acc) else {
            self.prev = None;
            return Ok(());
        };
        if let Some((prev, _)) = self.prev
            && prev.is(acc)
        {
            return Ok(());
        }
        match self.bound(top) {
            Some(depth) => {
                self.prev = Some((top, depth));
                Ok(())
            }
            None => Err(Error::invalid_arguments(format!(
                "reduce: the accumulator nests deeper than {MAX_ACCUMULATOR_DEPTH} levels"
            ))),
        }
    }

    /// A depth bound for `top`, or `None` past the cap.
    fn bound(&self, top: Container<'a>) -> Option<u32> {
        let mut deepest = 0;
        let mut first_new = 0;
        // The common rebuilt list: the previous list's elements, in order,
        // then new ones. One tight pass skips every element that is the
        // same value as before (a scalar, or the very same slice).
        if let (Container::Array(items), Some((Container::Array(prev_items), depth))) =
            (top, self.prev)
        {
            for (item, prev_item) in items.iter().zip(prev_items) {
                if is_container(item) {
                    if !same_slice(item, prev_item) {
                        break;
                    }
                    deepest = depth - 1;
                }
                first_new += 1;
            }
        }
        for at in first_new..top.len() {
            let child = top.child(at);
            if !is_container(child) {
                continue;
            }
            let depth = match self.prev {
                Some((prev, depth)) if prev.is(child) => depth,
                Some((prev, depth)) if at < prev.len() && same_slice(prev.child(at), child) => {
                    depth - 1
                }
                _ => self.walk(child, 1)?,
            };
            deepest = deepest.max(depth);
            if deepest >= MAX_ACCUMULATOR_DEPTH {
                return None;
            }
        }
        Some(deepest + 1)
    }

    /// Depth of `value`, which sits `level` levels down, or `None` once
    /// the two together pass the cap. The last measured accumulator, met
    /// on the way, counts at its bound. Recursion stops at the cap, so it
    /// is bounded however deep `value` is.
    fn walk(&self, value: &DataValue<'a>, level: u32) -> Option<u32> {
        if level > MAX_ACCUMULATOR_DEPTH {
            return None;
        }
        if let Some((prev, depth)) = self.prev
            && prev.is(value)
        {
            // `bound` checks the total against the cap.
            return Some(depth);
        }
        let mut deepest = 0;
        match value {
            DataValue::Array(items) => {
                for item in *items {
                    if is_container(item) {
                        deepest = deepest.max(self.walk(item, level + 1)?);
                    }
                }
            }
            DataValue::Object(pairs) => {
                for (_, item) in *pairs {
                    if is_container(item) {
                        deepest = deepest.max(self.walk(item, level + 1)?);
                    }
                }
            }
            _ => return Some(0),
        }
        Some(deepest + 1)
    }
}

/// An upper bound on how many levels one evaluation of `node` can add
/// above the deepest value it reads (the accumulator, the element, the
/// data), or `None` for a custom operator, which returns whatever the host
/// builds. A literal adds its own depth, an array or object literal one
/// level, and a built-in at most two (`group_by` and `entries` wrap
/// twice). Children's bounds add up rather than take the largest, since
/// one child's result can feed another: an iterator's body reads its
/// input's elements, and `try`'s fallback reads what its first argument
/// threw.
///
/// Two results are bounded outright rather than relative to what they
/// read: a nested `reduce`'s, by its own cap, and a tensor's (rank at most
/// 256, and it holds only numbers). Either can make one step jump to a
/// few hundred levels past that cap, but not keep climbing.
pub(super) fn growth(node: &CompiledNode) -> Option<u32> {
    let own: u32 = match node {
        CompiledNode::Value { value, .. } => return Some(literal_depth(value)),
        CompiledNode::CustomOperator(_) => return None,
        CompiledNode::Array { .. } => 1,
        #[cfg(feature = "templating")]
        CompiledNode::StructuredObject(_) => 1,
        #[cfg(feature = "error-handling")]
        CompiledNode::Throw(data) => return Some(literal_depth(&data.error) + 1),
        CompiledNode::Cse(_) => 0,
        _ => 2,
    };
    let mut total = own;
    let mut bounded = true;
    node.visit_indexed_children(&mut |_, child| match growth(child) {
        Some(g) => total = total.saturating_add(g),
        None => bounded = false,
    });
    bounded.then_some(total)
}

/// Nesting depth of a literal: 0 for a scalar, 1 for a flat array.
fn literal_depth(value: &OwnedDataValue) -> u32 {
    match value {
        OwnedDataValue::Array(items) => 1 + items.iter().map(literal_depth).max().unwrap_or(0),
        OwnedDataValue::Object(pairs) => {
            1 + pairs
                .iter()
                .map(|(_, v)| literal_depth(v))
                .max()
                .unwrap_or(0)
        }
        _ => 0,
    }
}

#[inline]
fn is_container(value: &DataValue<'_>) -> bool {
    matches!(value, DataValue::Array(_) | DataValue::Object(_))
}

/// Whether two values are the same container slice.
#[inline]
fn same_slice(a: &DataValue<'_>, b: &DataValue<'_>) -> bool {
    match (*a, *b) {
        (DataValue::Array(a), DataValue::Array(b)) => std::ptr::eq(a, b),
        (DataValue::Object(a), DataValue::Object(b)) => std::ptr::eq(a, b),
        _ => false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn body() -> CompiledNode {
        CompiledNode::synthetic_value(OwnedDataValue::Null)
    }

    /// `levels` arrays, each holding the next.
    fn nested<'a>(arena: &'a bumpalo::Bump, levels: u32) -> &'a DataValue<'a> {
        let mut v: &DataValue<'_> = arena.alloc(DataValue::Null);
        for _ in 0..levels {
            v = arena.alloc(DataValue::Array(arena.alloc_slice_copy(&[*v])));
        }
        v
    }

    #[test]
    fn scalars_and_shallow_values_pass() {
        let arena = bumpalo::Bump::new();
        let body = CompiledNode::synthetic_value(OwnedDataValue::Null);
        let mut d = AccumulatorDepth::new(&body);
        assert!(d.step(&DataValue::Null).is_ok());
        assert!(
            d.stride.is_none(),
            "nothing worked out for a scalar accumulator"
        );
        assert!(d.measure(nested(&arena, 3)).is_ok());
    }

    #[test]
    fn the_cap_is_inclusive() {
        let arena = bumpalo::Bump::new();
        assert!(
            AccumulatorDepth::new(&body())
                .measure(nested(&arena, MAX_ACCUMULATOR_DEPTH))
                .is_ok()
        );
        assert!(
            AccumulatorDepth::new(&body())
                .measure(nested(&arena, MAX_ACCUMULATOR_DEPTH + 1))
                .is_err()
        );
    }

    #[test]
    fn a_remembered_part_still_counts_toward_the_cap() {
        let arena = bumpalo::Bump::new();
        let body = body();
        let mut d = AccumulatorDepth::new(&body);
        let inner = nested(&arena, MAX_ACCUMULATOR_DEPTH);
        assert!(d.measure(inner).is_ok());
        // Wrapping the remembered value once more passes the cap.
        let outer: &DataValue<'_> =
            arena.alloc(DataValue::Array(arena.alloc_slice_copy(&[*inner])));
        assert!(d.measure(outer).is_err());
    }

    #[test]
    fn an_empty_container_at_the_cap_counts() {
        let arena = bumpalo::Bump::new();
        // MAX - 1 wraps around an empty array: MAX levels in all.
        let mut v: &DataValue<'_> = arena.alloc(DataValue::Array(&[]));
        for _ in 0..MAX_ACCUMULATOR_DEPTH - 1 {
            v = arena.alloc(DataValue::Array(arena.alloc_slice_copy(&[*v])));
        }
        assert!(AccumulatorDepth::new(&body()).measure(v).is_ok());
        let v: &DataValue<'_> = arena.alloc(DataValue::Array(arena.alloc_slice_copy(&[*v])));
        assert!(AccumulatorDepth::new(&body()).measure(v).is_err());
    }

    #[test]
    fn far_deeper_values_error_without_deep_recursion() {
        let arena = bumpalo::Bump::new();
        assert!(
            AccumulatorDepth::new(&body())
                .measure(nested(&arena, 200_000))
                .is_err()
        );
    }
}
