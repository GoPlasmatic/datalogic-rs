//! `ArenaContextStack` — arena-mode mirror of `ContextStack`.
//!
//! Frames hold `&'a ArenaValue<'a>` instead of owned `Value`. The root data
//! is borrowed from the caller's `Arc<Value>` (held by the `evaluate()` call
//! frame for the lifetime `'a`).
//!
//! Per-iteration cost: pushing a frame is `frames.push(...)` of two pointers
//! (no `Value::clone`, no `BTreeMap::clone`). The win Phase 1 unlocks for
//! Phase 5's collection-op migration.

use serde_json::Value;

use super::value::ArenaValue;

#[cfg(test)]
use bumpalo::Bump;

/// A single frame in the arena-mode context stack.
#[derive(Clone, Copy)]
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

    #[inline]
    pub(crate) fn get_index(&self) -> Option<usize> {
        match self {
            Self::Indexed { index, .. } | Self::Keyed { index, .. } => Some(*index),
            _ => None,
        }
    }

    #[inline]
    pub(crate) fn get_key(&self) -> Option<&'a str> {
        match self {
            Self::Keyed { key, .. } => Some(key),
            _ => None,
        }
    }

    #[inline]
    pub(crate) fn get_reduce_current(&self) -> Option<&'a ArenaValue<'a>> {
        match self {
            Self::Reduce { current, .. } => Some(current),
            _ => None,
        }
    }

    #[inline]
    pub(crate) fn get_reduce_accumulator(&self) -> Option<&'a ArenaValue<'a>> {
        match self {
            Self::Reduce { accumulator, .. } => Some(accumulator),
            _ => None,
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
    pub(crate) fn get_index(&self) -> Option<usize> {
        match self {
            Self::Frame(f) => f.get_index(),
            _ => None,
        }
    }

    #[inline]
    pub(crate) fn get_key(&self) -> Option<&'a str> {
        match self {
            Self::Frame(f) => f.get_key(),
            _ => None,
        }
    }

    #[cfg(test)]
    #[inline]
    pub(crate) fn data_input_ref(&self) -> Option<&'a Value> {
        match self {
            Self::Root(v) => Some(*v),
            Self::Frame(_) => None,
        }
    }

    #[cfg(test)]
    #[inline]
    pub(crate) fn frame_data(&self) -> Option<&'a ArenaValue<'a>> {
        match self {
            Self::Frame(f) => Some(f.data()),
            Self::Root(_) => None,
        }
    }
}

/// Arena-mode context stack. The lifetime `'a` is the arena lifetime; the
/// root borrows from the caller's `Arc<Value>` for the same `'a`.
pub struct ArenaContextStack<'a> {
    root: &'a Value,
    frames: Vec<ArenaContextFrame<'a>>,
    /// Breadcrumb of `CompiledNode::id`s accumulated as errors unwind.
    /// Mirrors `ContextStack::error_path`.
    error_path: Vec<u32>,
}

impl<'a> ArenaContextStack<'a> {
    #[inline]
    pub(crate) fn new(root: &'a Value) -> Self {
        Self {
            root,
            frames: Vec::new(),
            error_path: Vec::new(),
        }
    }

    /// Get the root input data (borrowed for the call's duration).
    #[inline]
    pub fn root_input(&self) -> &'a Value {
        self.root
    }

    /// Current depth (number of pushed iteration frames).
    #[inline]
    pub fn depth(&self) -> usize {
        self.frames.len()
    }

    /// Get the current context (top frame, or root if empty).
    #[inline]
    pub(crate) fn current(&self) -> ArenaContextRef<'a, '_> {
        if let Some(frame) = self.frames.last() {
            ArenaContextRef::Frame(frame)
        } else {
            ArenaContextRef::Root(self.root)
        }
    }

    /// Get the root context.
    #[inline]
    #[allow(dead_code)]
    pub(crate) fn root(&self) -> ArenaContextRef<'a, '_> {
        ArenaContextRef::Root(self.root)
    }

    /// Walk `level` frames up from the current context. Negative/positive
    /// magnitudes treated as absolute (matches `ContextStack::get_at_level`).
    pub(crate) fn get_at_level(&self, level: isize) -> Option<ArenaContextRef<'a, '_>> {
        let levels_up = level.unsigned_abs();
        if levels_up == 0 {
            return Some(self.current());
        }
        let frame_count = self.frames.len();
        if levels_up >= frame_count {
            return Some(ArenaContextRef::Root(self.root));
        }
        let target_index = frame_count - levels_up;
        self.frames.get(target_index).map(ArenaContextRef::Frame)
    }

    // ----- frame mutation ---------------------------------------------------

    #[inline]
    #[allow(dead_code)]
    pub(crate) fn push(&mut self, data: &'a ArenaValue<'a>) {
        self.frames.push(ArenaContextFrame::Data(data));
    }

    #[inline]
    pub(crate) fn push_with_index(&mut self, data: &'a ArenaValue<'a>, index: usize) {
        self.frames.push(ArenaContextFrame::Indexed { data, index });
    }

    #[inline]
    pub(crate) fn push_with_key_index(
        &mut self,
        data: &'a ArenaValue<'a>,
        index: usize,
        key: &'a str,
    ) {
        self.frames.push(ArenaContextFrame::Keyed { data, index, key });
    }

    #[inline]
    pub(crate) fn push_reduce(
        &mut self,
        current: &'a ArenaValue<'a>,
        accumulator: &'a ArenaValue<'a>,
    ) {
        self.frames
            .push(ArenaContextFrame::Reduce { current, accumulator });
    }

    #[inline]
    pub(crate) fn replace_top_data(&mut self, data: &'a ArenaValue<'a>, index: usize) {
        if let Some(frame) = self.frames.last_mut() {
            *frame = ArenaContextFrame::Indexed { data, index };
        }
    }

    #[inline]
    pub(crate) fn replace_top_key_data(
        &mut self,
        data: &'a ArenaValue<'a>,
        index: usize,
        key: &'a str,
    ) {
        if let Some(frame) = self.frames.last_mut() {
            *frame = ArenaContextFrame::Keyed { data, index, key };
        }
    }

    #[inline]
    pub(crate) fn replace_reduce_data(
        &mut self,
        current: &'a ArenaValue<'a>,
        accumulator: &'a ArenaValue<'a>,
    ) {
        if let Some(frame) = self.frames.last_mut() {
            *frame = ArenaContextFrame::Reduce { current, accumulator };
        }
    }

    #[inline]
    pub(crate) fn pop(&mut self) -> Option<ArenaContextFrame<'a>> {
        self.frames.pop()
    }

    // ----- error breadcrumb (mirrors ContextStack) --------------------------

    #[inline]
    #[allow(dead_code)]
    pub(crate) fn push_error_step(&mut self, id: u32) {
        self.error_path.push(id);
    }

    #[inline]
    #[allow(dead_code)]
    pub(crate) fn error_path_len(&self) -> usize {
        self.error_path.len()
    }

    #[inline]
    #[allow(dead_code)]
    pub(crate) fn truncate_error_path(&mut self, len: usize) {
        self.error_path.truncate(len);
    }

    #[inline]
    #[allow(dead_code)]
    pub(crate) fn take_error_path(&mut self) -> Vec<u32> {
        std::mem::take(&mut self.error_path)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::arena::value::ArenaValue;

    #[test]
    fn lifecycle_indexed() {
        let arena = Bump::new();
        let root_val = Value::Null;
        let mut ctx = ArenaContextStack::new(&root_val);
        assert_eq!(ctx.depth(), 0);
        assert!(ctx.current().data_input_ref().is_some(), "root at depth 0");

        let a: &ArenaValue = arena.alloc(ArenaValue::Number(crate::value::NumberValue::from_i64(1)));
        ctx.push_with_index(a, 0);
        assert_eq!(ctx.depth(), 1);
        assert_eq!(ctx.current().get_index(), Some(0));

        let b: &ArenaValue = arena.alloc(ArenaValue::Number(crate::value::NumberValue::from_i64(2)));
        ctx.replace_top_data(b, 1);
        assert_eq!(ctx.current().get_index(), Some(1));

        ctx.pop();
        assert_eq!(ctx.depth(), 0);
    }

    #[test]
    fn lifecycle_keyed() {
        let arena = Bump::new();
        let root_val = Value::Null;
        let mut ctx = ArenaContextStack::new(&root_val);

        let a: &ArenaValue = arena.alloc(ArenaValue::Bool(true));
        ctx.push_with_key_index(a, 0, "k1");
        assert_eq!(ctx.current().get_key(), Some("k1"));

        let b: &ArenaValue = arena.alloc(ArenaValue::Bool(false));
        ctx.replace_top_key_data(b, 1, "k2");
        assert_eq!(ctx.current().get_key(), Some("k2"));
        assert_eq!(ctx.current().get_index(), Some(1));
    }

    #[test]
    fn lifecycle_reduce() {
        let arena = Bump::new();
        let root_val = Value::Null;
        let mut ctx = ArenaContextStack::new(&root_val);

        let cur: &ArenaValue = arena.alloc(ArenaValue::Number(crate::value::NumberValue::from_i64(1)));
        let acc: &ArenaValue = arena.alloc(ArenaValue::Number(crate::value::NumberValue::from_i64(0)));
        ctx.push_reduce(cur, acc);
        assert_eq!(ctx.depth(), 1);

        if let ArenaContextRef::Frame(f) = ctx.current() {
            assert!(f.get_reduce_current().is_some());
            assert!(f.get_reduce_accumulator().is_some());
        } else {
            panic!("expected frame");
        }
    }

    #[test]
    fn get_at_level_walks_up() {
        let arena = Bump::new();
        let root_val = Value::Null;
        let mut ctx = ArenaContextStack::new(&root_val);

        let a: &ArenaValue = arena.alloc(ArenaValue::Number(crate::value::NumberValue::from_i64(10)));
        let b: &ArenaValue = arena.alloc(ArenaValue::Number(crate::value::NumberValue::from_i64(20)));
        ctx.push_with_index(a, 0);
        ctx.push_with_index(b, 0);
        assert_eq!(ctx.depth(), 2);

        // Level 0 = current (b)
        assert!(ctx.get_at_level(0).and_then(|r| r.frame_data()).is_some());
        // Level 1 = parent (a)
        assert!(ctx.get_at_level(1).and_then(|r| r.frame_data()).is_some());
        // Level 2 = root
        assert!(ctx.get_at_level(2).and_then(|r| r.data_input_ref()).is_some());
        // Level 5 (overflow) = root
        assert!(ctx.get_at_level(5).and_then(|r| r.data_input_ref()).is_some());
    }

    #[test]
    fn error_path_round_trip() {
        let arena = Bump::new();
        let root_val = Value::Null;
        let mut ctx = ArenaContextStack::new(&root_val);

        ctx.push_error_step(1);
        ctx.push_error_step(2);
        ctx.push_error_step(3);
        assert_eq!(ctx.error_path_len(), 3);

        ctx.truncate_error_path(1);
        let p = ctx.take_error_path();
        assert_eq!(p, vec![1]);
    }
}
