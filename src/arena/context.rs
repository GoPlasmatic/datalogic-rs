//! `ArenaContextStack` — context stack used during arena-mode evaluation.
//!
//! Frames hold `&'a ArenaValue<'a>`, and so does the root: callers either
//! pass an arena-resident value directly (e.g. `evaluate_in_arena`) or use
//! `from_value` to wrap a borrowed `&Value` as `InputRef` in the arena.
//!
//! Per-iteration cost: pushing a frame is `frames.push(...)` of two pointers
//! (no `Value::clone`, no `BTreeMap::clone`).

use super::value::ArenaValue;
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
    /// Root carries the original input as `&'a ArenaValue<'a>`. Typically the
    /// caller wraps a `&Value` as `InputRef` (zero-copy borrow); benchmarks
    /// and other arena-native callers may pass a fully arena-resident value.
    Root(&'a ArenaValue<'a>),
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
    pub(crate) fn root_data(&self) -> Option<&'a ArenaValue<'a>> {
        match self {
            Self::Root(av) => Some(*av),
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
/// root is `&'a ArenaValue<'a>` (an `InputRef` wrapper for `&Value` callers,
/// or a fully arena-resident value for arena-native callers).
pub struct ArenaContextStack<'a> {
    root: &'a ArenaValue<'a>,
    frames: Vec<ArenaContextFrame<'a>>,
    /// Breadcrumb of `CompiledNode::id`s accumulated as errors unwind.
    /// Mirrors `ContextStack::error_path`.
    error_path: Vec<u32>,
    /// Optional trace collector, set when this stack drives a traced
    /// evaluation. Stored as a raw pointer so the stack stays free of an
    /// extra lifetime parameter; the caller (`evaluate_*_with_trace`) keeps
    /// a `&mut TraceCollector` live for the entire call, so dereferencing
    /// the pointer is sound.
    #[cfg(feature = "trace")]
    tracer: Option<std::ptr::NonNull<crate::trace::TraceCollector>>,
}

impl<'a> ArenaContextStack<'a> {
    #[inline]
    pub(crate) fn new(root: &'a ArenaValue<'a>) -> Self {
        Self {
            root,
            frames: Vec::new(),
            error_path: Vec::new(),
            #[cfg(feature = "trace")]
            tracer: None,
        }
    }

    /// Build a context stack from a borrowed `&Value`, wrapping it as an
    /// arena-allocated `InputRef`. The wrapper is one allocation — no deep
    /// copy of the input — so this is the canonical entry point for the
    /// `&Value`-based public APIs.
    #[inline]
    pub(crate) fn from_value(root: &'a serde_json::Value, arena: &'a Bump) -> Self {
        Self::new(arena.alloc(ArenaValue::InputRef(root)))
    }

    /// Attach a trace collector to this stack. Caller must keep the
    /// `&mut TraceCollector` live for the duration of the evaluation that
    /// uses this stack — the stack stores a raw pointer to it.
    #[cfg(feature = "trace")]
    #[inline]
    pub(crate) fn set_tracer(&mut self, tracer: &mut crate::trace::TraceCollector) {
        self.tracer = std::ptr::NonNull::new(tracer as *mut _);
    }

    /// True iff a tracer has been attached.
    #[cfg(feature = "trace")]
    #[inline]
    pub(crate) fn has_tracer(&self) -> bool {
        self.tracer.is_some()
    }

    /// Snapshot the current frame's data as an owned `Value`. Used by the
    /// arena dispatcher before recursing into a child, so the trace step
    /// can record the context that operator saw.
    #[cfg(feature = "trace")]
    pub(crate) fn current_data_as_value(&self) -> serde_json::Value {
        match self.current() {
            ArenaContextRef::Root(av) => crate::arena::arena_to_value(av),
            ArenaContextRef::Frame(f) => crate::arena::arena_to_value(f.data()),
        }
    }

    /// Record the result of a node into the attached tracer. No-op if no
    /// tracer is attached. Callers gate on [`has_tracer`] first to skip the
    /// `Value::clone()` when not tracing.
    #[cfg(feature = "trace")]
    pub(crate) fn record_node_result(
        &mut self,
        node_id: u32,
        ctx_data: serde_json::Value,
        result: &crate::Result<&'a crate::arena::ArenaValue<'a>>,
    ) {
        let Some(ptr) = self.tracer else {
            return;
        };
        // SAFETY: the tracer pointer was set via `set_tracer(&mut TraceCollector)`
        // and the caller keeps that mutable borrow live for the full evaluation.
        let collector = unsafe { ptr.as_ptr().as_mut().expect("non-null") };
        match result {
            Ok(av) => {
                let v = crate::arena::arena_to_value(av);
                collector.record_step(node_id, ctx_data, v);
            }
            Err(e) => {
                collector.record_error(node_id, ctx_data, e.to_string());
            }
        }
    }

    /// Mark entry into an iteration body — drives the per-step
    /// `iteration_index` / `iteration_total` fields on traced steps.
    #[cfg(feature = "trace")]
    #[inline]
    pub(crate) fn trace_push_iteration(&mut self, index: u32, total: u32) {
        if let Some(ptr) = self.tracer {
            let collector = unsafe { ptr.as_ptr().as_mut().expect("non-null") };
            collector.push_iteration(index, total);
        }
    }

    /// Mark exit from an iteration body.
    #[cfg(feature = "trace")]
    #[inline]
    pub(crate) fn trace_pop_iteration(&mut self) {
        if let Some(ptr) = self.tracer {
            let collector = unsafe { ptr.as_ptr().as_mut().expect("non-null") };
            collector.pop_iteration();
        }
    }

    /// Get the root input data (borrowed for the call's duration).
    #[inline]
    pub fn root_input(&self) -> &'a ArenaValue<'a> {
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
        self.frames
            .push(ArenaContextFrame::Keyed { data, index, key });
    }

    #[inline]
    pub(crate) fn push_reduce(
        &mut self,
        current: &'a ArenaValue<'a>,
        accumulator: &'a ArenaValue<'a>,
    ) {
        self.frames.push(ArenaContextFrame::Reduce {
            current,
            accumulator,
        });
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
            *frame = ArenaContextFrame::Reduce {
                current,
                accumulator,
            };
        }
    }

    #[inline]
    pub(crate) fn pop(&mut self) -> Option<ArenaContextFrame<'a>> {
        self.frames.pop()
    }

    // ----- error breadcrumb (mirrors ContextStack) --------------------------

    #[cold]
    #[inline(never)]
    pub(crate) fn push_error_step(&mut self, id: u32) {
        self.error_path.push(id);
    }

    #[inline]
    pub(crate) fn error_path_len(&self) -> usize {
        self.error_path.len()
    }

    #[inline]
    pub(crate) fn truncate_error_path(&mut self, len: usize) {
        self.error_path.truncate(len);
    }

    #[inline]
    pub(crate) fn take_error_path(&mut self) -> Vec<u32> {
        std::mem::take(&mut self.error_path)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::arena::value::ArenaValue;
    use serde_json::Value;

    #[test]
    fn lifecycle_indexed() {
        let arena = Bump::new();
        let root_val = Value::Null;
        let mut ctx = ArenaContextStack::from_value(&root_val, &arena);
        assert_eq!(ctx.depth(), 0);
        assert!(ctx.current().root_data().is_some(), "root at depth 0");

        let a: &ArenaValue =
            arena.alloc(ArenaValue::Number(crate::value::NumberValue::from_i64(1)));
        ctx.push_with_index(a, 0);
        assert_eq!(ctx.depth(), 1);
        assert_eq!(ctx.current().get_index(), Some(0));

        let b: &ArenaValue =
            arena.alloc(ArenaValue::Number(crate::value::NumberValue::from_i64(2)));
        ctx.replace_top_data(b, 1);
        assert_eq!(ctx.current().get_index(), Some(1));

        ctx.pop();
        assert_eq!(ctx.depth(), 0);
    }

    #[test]
    fn lifecycle_keyed() {
        let arena = Bump::new();
        let root_val = Value::Null;
        let mut ctx = ArenaContextStack::from_value(&root_val, &arena);

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
        let mut ctx = ArenaContextStack::from_value(&root_val, &arena);

        let cur: &ArenaValue =
            arena.alloc(ArenaValue::Number(crate::value::NumberValue::from_i64(1)));
        let acc: &ArenaValue =
            arena.alloc(ArenaValue::Number(crate::value::NumberValue::from_i64(0)));
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
        let mut ctx = ArenaContextStack::from_value(&root_val, &arena);

        let a: &ArenaValue =
            arena.alloc(ArenaValue::Number(crate::value::NumberValue::from_i64(10)));
        let b: &ArenaValue =
            arena.alloc(ArenaValue::Number(crate::value::NumberValue::from_i64(20)));
        ctx.push_with_index(a, 0);
        ctx.push_with_index(b, 0);
        assert_eq!(ctx.depth(), 2);

        // Level 0 = current (b)
        assert!(ctx.get_at_level(0).and_then(|r| r.frame_data()).is_some());
        // Level 1 = parent (a)
        assert!(ctx.get_at_level(1).and_then(|r| r.frame_data()).is_some());
        // Level 2 = root
        assert!(ctx.get_at_level(2).and_then(|r| r.root_data()).is_some());
        // Level 5 (overflow) = root
        assert!(ctx.get_at_level(5).and_then(|r| r.root_data()).is_some());
    }

    #[test]
    fn error_path_round_trip() {
        let arena = Bump::new();
        let root_val = Value::Null;
        let mut ctx = ArenaContextStack::from_value(&root_val, &arena);

        ctx.push_error_step(1);
        ctx.push_error_step(2);
        ctx.push_error_step(3);
        assert_eq!(ctx.error_path_len(), 3);

        ctx.truncate_error_path(1);
        let p = ctx.take_error_path();
        assert_eq!(p, vec![1]);
    }
}
