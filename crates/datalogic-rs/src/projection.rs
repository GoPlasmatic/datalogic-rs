//! Read projection: bring into the arena only the parts of an input a
//! compiled rule reads.
//!
//! Evaluating against an owned or `serde_json` value builds an arena view
//! of it. A view of the whole input costs a spine over every container in
//! it, however little the rule reads. When the rule's reads are all known
//! ([`Facts::reads_complete`](crate::Facts::reads_complete)) and none is
//! the whole input, the evaluator needs only the values at those paths, so
//! the view keeps, on the way down each path, just the object keys the
//! path names, and views the whole value at the path's end.
//!
//! The view a rule sees answers every read it makes exactly as the whole
//! input would:
//!
//! - a read of path `p` finds the whole value at `p`, everything under it
//!   included;
//! - an object above a read path keeps every entry whose key the path
//!   names, in input order and with duplicates, so a key lookup finds what
//!   it finds on the whole object; no read observes such an object whole,
//!   or its path would be a read and it would be kept whole;
//! - a read path that meets an array or a scalar before its end keeps that
//!   value whole, since an array index or a missing key below a scalar
//!   resolves on it as on the whole input.
//!
//! `tests/projection_test.rs` checks this against every suite case.

use bumpalo::Bump;
use datavalue::OwnedDataValue;

use crate::Facts;
use crate::arena::DataValue;

/// The longest read path a projection is built for; see [`Projection::of`].
const MAX_PATH_SEGMENTS: usize = crate::node::MAX_COMPILE_DEPTH;

/// The paths a rule reads, as a trie: each node is what is read under one
/// key. [`Projection::of`] is `None` when an evaluation needs the whole
/// input.
#[derive(Debug, Clone, Default)]
pub(crate) struct Projection {
    /// A read ends here: keep the whole value.
    whole: bool,
    /// The keys read below this point, each with what is read under it,
    /// sorted by key.
    children: Vec<(Box<str>, Projection)>,
}

impl Projection {
    /// What of an input a rule with `facts` reads, or `None` when it may
    /// read anything (a computed path, a custom operator that reads the
    /// context) or reads the whole input.
    ///
    /// Also `None` for a path longer than [`MAX_PATH_SEGMENTS`]: building,
    /// walking and dropping the trie recurse once per segment, and a path's
    /// length is bounded only by the rule's size, so a very long one is
    /// read from the whole input instead.
    pub(crate) fn of(facts: &Facts) -> Option<Self> {
        if !facts.reads_complete() {
            return None;
        }
        let mut root = Projection::default();
        for path in facts.reads() {
            if path.is_root() || path.segments().len() > MAX_PATH_SEGMENTS {
                return None;
            }
            root.insert(path.segments());
        }
        Some(root)
    }

    /// What is read under key `key`, or `None` when nothing is. Called
    /// once per entry of each projected input object, and for each part of
    /// a [`crate::Roots`] input.
    ///
    /// `children` is kept sorted by key, so a wide read set is searched in
    /// O(log n) rather than scanned for every input key; a narrow one (the
    /// common case) is still scanned, which is faster at that size.
    #[inline]
    pub(crate) fn under(&self, key: &str) -> Option<&Projection> {
        const SCAN_MAX: usize = 8;
        if self.children.len() <= SCAN_MAX {
            return self
                .children
                .iter()
                .find(|(k, _)| **k == *key)
                .map(|(_, n)| n);
        }
        self.children
            .binary_search_by(|(k, _)| (**k).cmp(key))
            .ok()
            .map(|at| &self.children[at].1)
    }

    fn insert(&mut self, segments: &[String]) {
        if self.whole {
            return;
        }
        let Some((first, rest)) = segments.split_first() else {
            self.whole = true;
            self.children = Vec::new();
            return;
        };
        // Insert in key order; see `under`.
        let at = match self
            .children
            .binary_search_by(|(k, _)| (**k).cmp(first.as_str()))
        {
            Ok(at) => at,
            Err(at) => {
                self.children
                    .insert(at, (first.as_str().into(), Projection::default()));
                at
            }
        };
        self.children[at].1.insert(rest);
    }

    /// The view of `input` the rule needs.
    pub(crate) fn owned<'a>(&self, input: &'a OwnedDataValue, arena: &'a Bump) -> DataValue<'a> {
        if self.whole {
            return input.view_in(arena);
        }
        match input {
            OwnedDataValue::Object(pairs) => {
                // Sized up front: each child's view is allocated between
                // pushes, so a growing `kept` would copy itself each time.
                let mut kept =
                    bumpalo::collections::Vec::with_capacity_in(self.children.len(), arena);
                for (key, value) in pairs {
                    if let Some(child) = self.under(key) {
                        kept.push((key.as_str(), child.owned(value, arena)));
                    }
                }
                // A wide object is looked up by an ordered probe, which on
                // a repeated key may find a later copy than the first-match
                // scan the narrower kept object gets. Rare (a document that
                // repeats a key the rule reads), so view such an object
                // whole rather than replicate the probe.
                if pairs.len() >= crate::arena::value::ORDERED_PROBE_MIN_PAIRS
                    && kept.len() > self.children.len()
                {
                    return input.view_in(arena);
                }
                DataValue::Object(kept.into_bump_slice())
            }
            other => other.view_in(arena),
        }
    }

    /// [`Self::owned`] for a `serde_json` input.
    #[cfg(feature = "serde_json")]
    pub(crate) fn serde<'a>(&self, input: &'a serde_json::Value, arena: &'a Bump) -> DataValue<'a> {
        if self.whole {
            return crate::arena::value_to_data(input, arena);
        }
        match input {
            // A map holds each key once, so look each read key up rather
            // than walk every entry of a wide object.
            serde_json::Value::Object(map) => {
                let mut kept =
                    bumpalo::collections::Vec::with_capacity_in(self.children.len(), arena);
                for (key, child) in &self.children {
                    if let Some((key, value)) = map.get_key_value(&**key) {
                        kept.push((key.as_str(), child.serde(value, arena)));
                    }
                }
                DataValue::Object(kept.into_bump_slice())
            }
            other => crate::arena::value_to_data(other, arena),
        }
    }
}
