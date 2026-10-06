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

/// The paths a rule reads, as a trie. [`Projection::of`] is `None` when an
/// evaluation needs the whole input.
#[derive(Debug, Clone, Default)]
pub(crate) struct Projection {
    root: Node,
}

#[derive(Debug, Clone, Default)]
struct Node {
    /// A read ends here: keep the whole value.
    whole: bool,
    /// The keys read below this point, each with what is read under it.
    children: Vec<(Box<str>, Node)>,
}

impl Projection {
    /// What of an input a rule with `facts` reads, or `None` when it may
    /// read anything (a computed path, a custom operator that reads the
    /// context) or reads the whole input.
    pub(crate) fn of(facts: &Facts) -> Option<Self> {
        if !facts.reads_complete() {
            return None;
        }
        let mut root = Node::default();
        for path in facts.reads() {
            if path.is_root() {
                return None;
            }
            root.insert(path.segments());
        }
        Some(Projection { root })
    }

    /// The view of `input` the rule needs.
    pub(crate) fn owned<'a>(&self, input: &'a OwnedDataValue, arena: &'a Bump) -> DataValue<'a> {
        self.root.owned(input, arena)
    }

    /// What is read under top-level key `key`, or `None` when nothing is:
    /// for an input whose top-level object is assembled from parts
    /// ([`crate::Roots`]).
    pub(crate) fn under(&self, key: &str) -> Option<Under<'_>> {
        self.root.child(key).map(Under)
    }

    /// [`Self::owned`] for a `serde_json` input.
    #[cfg(feature = "serde_json")]
    pub(crate) fn serde<'a>(&self, input: &'a serde_json::Value, arena: &'a Bump) -> DataValue<'a> {
        self.root.serde(input, arena)
    }
}

/// The part of a [`Projection`] under one top-level key.
#[derive(Debug, Clone, Copy)]
pub(crate) struct Under<'p>(&'p Node);

impl Under<'_> {
    /// The view of `input`, the value under the key, the rule needs.
    pub(crate) fn owned<'a>(self, input: &'a OwnedDataValue, arena: &'a Bump) -> DataValue<'a> {
        self.0.owned(input, arena)
    }

    /// [`Self::owned`] for a `serde_json` value.
    #[cfg(feature = "serde_json")]
    pub(crate) fn serde<'a>(self, input: &'a serde_json::Value, arena: &'a Bump) -> DataValue<'a> {
        self.0.serde(input, arena)
    }
}

impl Node {
    fn insert(&mut self, segments: &[String]) {
        if self.whole {
            return;
        }
        let Some((first, rest)) = segments.split_first() else {
            self.whole = true;
            self.children = Vec::new();
            return;
        };
        let at = match self.children.iter().position(|(k, _)| **k == **first) {
            Some(at) => at,
            None => {
                self.children.push((first.as_str().into(), Node::default()));
                self.children.len() - 1
            }
        };
        self.children[at].1.insert(rest);
    }

    #[inline]
    fn child(&self, key: &str) -> Option<&Node> {
        self.children
            .iter()
            .find(|(k, _)| **k == *key)
            .map(|(_, n)| n)
    }

    fn owned<'a>(&self, input: &'a OwnedDataValue, arena: &'a Bump) -> DataValue<'a> {
        if self.whole {
            return input.view_in(arena);
        }
        match input {
            OwnedDataValue::Object(pairs) => {
                let mut kept = bumpalo::collections::Vec::new_in(arena);
                for (key, value) in pairs {
                    if let Some(child) = self.child(key) {
                        kept.push((key.as_str(), child.owned(value, arena)));
                    }
                }
                DataValue::Object(kept.into_bump_slice())
            }
            other => other.view_in(arena),
        }
    }

    #[cfg(feature = "serde_json")]
    fn serde<'a>(&self, input: &'a serde_json::Value, arena: &'a Bump) -> DataValue<'a> {
        if self.whole {
            return crate::arena::value_to_data(input, arena);
        }
        match input {
            // A map holds each key once, so look each read key up rather
            // than walk every entry of a wide object.
            serde_json::Value::Object(map) => {
                let mut kept = bumpalo::collections::Vec::new_in(arena);
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
