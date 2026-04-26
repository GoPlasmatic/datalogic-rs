//! Minimal value-mode context — kept only for the dynamic-path raw `var`/
//! `val`/`exists` helpers in `src/operators/variable.rs` that haven't yet
//! been ported to native arena dispatch. Arena dispatch carries its own
//! state via [`crate::arena::ArenaContextStack`].
//!
//! `ContextStack` here always holds a single root frame (no iter frames).
//! All `push_*` / `pop` / `replace_*` methods on the legacy interface are
//! gone; the only operations are `new(root)` and `current().data()`.

use serde_json::Value;
use std::sync::Arc;

/// Root-only reference returned by [`ContextStack::current`]. Provided for
/// API compatibility with the legacy `ContextFrameRef` shape; the iter-frame
/// variants no longer exist because no caller pushes frames.
pub struct ContextFrameRef<'a> {
    root: &'a Arc<Value>,
}

impl<'a> ContextFrameRef<'a> {
    #[inline]
    pub fn data(&self) -> &Value {
        self.root
    }

    #[inline]
    pub fn get_index(&self) -> Option<usize> {
        None
    }

    #[inline]
    pub fn get_key(&self) -> Option<&str> {
        None
    }

    #[inline]
    pub fn get_reduce_current(&self) -> Option<&Value> {
        None
    }

    #[inline]
    pub fn get_reduce_accumulator(&self) -> Option<&Value> {
        None
    }
}

/// Root-only context for legacy value-mode helpers.
pub struct ContextStack {
    root: Arc<Value>,
}

impl ContextStack {
    /// Create a new root-only context.
    #[inline]
    pub fn new(root: Arc<Value>) -> Self {
        Self { root }
    }

    /// Get the root context frame.
    #[inline]
    pub fn current(&self) -> ContextFrameRef<'_> {
        ContextFrameRef { root: &self.root }
    }

    /// Walk `level` frames up from the current context. With root-only
    /// state, every level resolves to the root.
    #[inline]
    pub fn get_at_level(&self, _level: isize) -> Option<ContextFrameRef<'_>> {
        Some(self.current())
    }
}
