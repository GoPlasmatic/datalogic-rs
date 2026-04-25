//! `ArenaContextStack` — arena-mode mirror of `ContextStack`.
//!
//! Frames hold `&'a ArenaValue<'a>` instead of owned `Value`. The root data
//! is borrowed from the caller's `Arc<Value>` (held by the `evaluate()` call
//! frame for the lifetime `'a`).
//!
//! POC scope: this type is reserved for Phase 4 (composition INTO arena —
//! when iterators consume `&[ArenaValue]` rather than `&[Value]`). Phase 2/3
//! reuse the existing `ContextStack` for predicate evaluation, so the API
//! here is intentionally unused right now. The definitions stay in-tree to
//! avoid re-litigating the design when Phase 4 lands.

#![allow(dead_code)] // forward-looking scaffolding for Phase 4

use bumpalo::Bump;
use serde_json::Value;

use super::value::ArenaValue;

/// A single frame in the arena-mode context stack.
pub(crate) enum ArenaContextFrame<'a> {
    Indexed {
        data: &'a ArenaValue<'a>,
        index: usize,
    },
    Keyed {
        data: &'a ArenaValue<'a>,
        index: usize,
        key: &'a str,
    },
    Reduce {
        current: &'a ArenaValue<'a>,
        accumulator: &'a ArenaValue<'a>,
    },
    Data(&'a ArenaValue<'a>),
}

impl<'a> ArenaContextFrame<'a> {
    #[inline]
    pub(crate) fn data(&self) -> &'a ArenaValue<'a> {
        match self {
            Self::Indexed { data, .. } | Self::Keyed { data, .. } | Self::Data(data) => data,
            Self::Reduce { current, .. } => current,
        }
    }
}

/// Reference to an arena context frame (either a stack frame or the root).
pub(crate) enum ArenaContextRef<'a, 'ctx> {
    Frame(&'ctx ArenaContextFrame<'a>),
    /// Root carries the original input as `&Value` so `var` lookups can return
    /// `InputRef` without wrapping in arena allocation.
    Root(&'a Value),
}

impl<'a, 'ctx> ArenaContextRef<'a, 'ctx> {
    #[inline]
    pub(crate) fn data_input_ref(&self) -> Option<&'a Value> {
        match self {
            Self::Root(v) => Some(*v),
            Self::Frame(_) => None,
        }
    }
}

/// Arena-mode context stack. The lifetime `'a` is the arena lifetime; the
/// root borrows from the caller's `Arc<Value>` for the same `'a`.
pub(crate) struct ArenaContextStack<'a> {
    root: &'a Value,
    frames: Vec<ArenaContextFrame<'a>>,
    /// Available for arena-allocating frame-local strings (object iteration).
    #[allow(dead_code)] // POC: used only when more operators are migrated
    pub(crate) arena: &'a Bump,
}

impl<'a> ArenaContextStack<'a> {
    #[inline]
    pub(crate) fn new(arena: &'a Bump, root: &'a Value) -> Self {
        Self {
            root,
            frames: Vec::new(),
            arena,
        }
    }

    /// Get the root input data (borrowed from the caller's Arc).
    #[inline]
    pub(crate) fn root_input(&self) -> &'a Value {
        self.root
    }

    /// Current depth (number of pushed frames). Depth 0 means current() == root.
    #[inline]
    #[allow(dead_code)] // POC: used by future operators
    pub(crate) fn depth(&self) -> usize {
        self.frames.len()
    }

    /// Get the current context (top frame, or root if empty).
    #[inline]
    #[allow(dead_code)] // POC: used by future operators
    pub(crate) fn current(&self) -> ArenaContextRef<'a, '_> {
        if let Some(frame) = self.frames.last() {
            ArenaContextRef::Frame(frame)
        } else {
            ArenaContextRef::Root(self.root)
        }
    }

    #[inline]
    #[allow(dead_code)] // POC: used by future operators
    pub(crate) fn push_indexed(&mut self, data: &'a ArenaValue<'a>, index: usize) {
        self.frames.push(ArenaContextFrame::Indexed { data, index });
    }

    #[inline]
    #[allow(dead_code)] // POC: used by future operators
    pub(crate) fn pop(&mut self) -> Option<ArenaContextFrame<'a>> {
        self.frames.pop()
    }
}
