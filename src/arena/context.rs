//! `DataContextStack` — context stack used during arena-mode evaluation.
//!
//! Frames hold `&'a DataValue<'a>`, and so does the root: callers either
//! pass an arena-resident value directly (e.g. `evaluate_in_arena`) or use
//! `from_value` to deep-convert a borrowed `&Value` into the arena.
//!
//! Per-iteration cost: pushing a frame is `frames.push(...)` of two pointers
//! (no `Value::clone`, no `BTreeMap::clone`).

use super::value::DataValue;
#[cfg(feature = "compat")]
use bumpalo::Bump;

/// A single frame in the arena-mode context stack.
#[derive(Clone, Copy)]
pub(crate) enum ArenaContextFrame<'a> {
    Indexed {
        data: &'a DataValue<'a>,
        index: usize,
    },
    Keyed {
        data: &'a DataValue<'a>,
        index: usize,
        key: &'a str,
    },
    Reduce {
        current: &'a DataValue<'a>,
        accumulator: &'a DataValue<'a>,
    },
    Data(&'a DataValue<'a>),
}

impl<'a> ArenaContextFrame<'a> {
    #[inline]
    pub(crate) fn data(&self) -> &'a DataValue<'a> {
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
    pub(crate) fn get_reduce_current(&self) -> Option<&'a DataValue<'a>> {
        match self {
            Self::Reduce { current, .. } => Some(current),
            _ => None,
        }
    }

    #[inline]
    pub(crate) fn get_reduce_accumulator(&self) -> Option<&'a DataValue<'a>> {
        match self {
            Self::Reduce { accumulator, .. } => Some(accumulator),
            _ => None,
        }
    }
}

/// Reference to an arena context frame (either a stack frame or the root).
pub(crate) enum ArenaContextRef<'a, 'ctx> {
    Frame(&'ctx ArenaContextFrame<'a>),
    /// Root carries the original input as `&'a DataValue<'a>`, deep-converted
    /// from a `&Value` at API entry or supplied directly by arena-native
    /// callers.
    Root(&'a DataValue<'a>),
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
    fn root_data(&self) -> Option<&'a DataValue<'a>> {
        match self {
            Self::Root(av) => Some(*av),
            Self::Frame(_) => None,
        }
    }

    #[cfg(test)]
    #[inline]
    fn frame_data(&self) -> Option<&'a DataValue<'a>> {
        match self {
            Self::Frame(f) => Some(f.data()),
            Self::Root(_) => None,
        }
    }
}

/// Arena-mode context stack. The lifetime `'a` is the arena lifetime; the
/// root is `&'a DataValue<'a>` (deep-converted from `&Value` for the public
/// API, or supplied directly by arena-native callers).
pub struct DataContextStack<'a> {
    root: &'a DataValue<'a>,
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

impl<'a> DataContextStack<'a> {
    #[inline]
    pub(crate) fn new(root: &'a DataValue<'a>) -> Self {
        Self {
            root,
            frames: Vec::new(),
            error_path: Vec::new(),
            #[cfg(feature = "trace")]
            tracer: None,
        }
    }

    /// Build a context stack from a borrowed `&serde_json::Value` by
    /// deep-converting it into an arena-resident `DataValue`. Bridge for the
    /// compat-mode public APIs only — v5 callers build a `DataValue` first
    /// and use [`DataContextStack::new`] directly.
    #[cfg(feature = "compat")]
    #[inline]
    pub(crate) fn from_value(root: &'a serde_json::Value, arena: &'a Bump) -> Self {
        let av = crate::arena::value::value_to_arena(root, arena);
        Self::new(arena.alloc(av))
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
    #[cfg(all(feature = "trace", feature = "compat"))]
    pub(crate) fn current_data_as_value(&self) -> serde_json::Value {
        match self.current() {
            ArenaContextRef::Root(av) => crate::arena::arena_to_value(av),
            ArenaContextRef::Frame(f) => crate::arena::arena_to_value(f.data()),
        }
    }

    /// Record the result of a node into the attached tracer. No-op if no
    /// tracer is attached. Callers gate on [`has_tracer`] first to skip the
    /// `Value::clone()` when not tracing.
    #[cfg(all(feature = "trace", feature = "compat"))]
    pub(crate) fn record_node_result(
        &mut self,
        node_id: u32,
        ctx_data: serde_json::Value,
        result: &crate::Result<&'a crate::arena::DataValue<'a>>,
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
    pub fn root_input(&self) -> &'a DataValue<'a> {
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
    pub(crate) fn push(&mut self, data: &'a DataValue<'a>) {
        self.frames.push(ArenaContextFrame::Data(data));
    }

    #[inline]
    pub(crate) fn push_with_index(&mut self, data: &'a DataValue<'a>, index: usize) {
        self.frames.push(ArenaContextFrame::Indexed { data, index });
    }

    #[inline]
    fn push_with_key_index(&mut self, data: &'a DataValue<'a>, index: usize, key: &'a str) {
        self.frames
            .push(ArenaContextFrame::Keyed { data, index, key });
    }

    #[inline]
    fn push_reduce(&mut self, current: &'a DataValue<'a>, accumulator: &'a DataValue<'a>) {
        self.frames.push(ArenaContextFrame::Reduce {
            current,
            accumulator,
        });
    }

    #[inline]
    fn replace_top_data(&mut self, data: &'a DataValue<'a>, index: usize) {
        if let Some(frame) = self.frames.last_mut() {
            *frame = ArenaContextFrame::Indexed { data, index };
        }
    }

    #[inline]
    fn replace_top_key_data(&mut self, data: &'a DataValue<'a>, index: usize, key: &'a str) {
        if let Some(frame) = self.frames.last_mut() {
            *frame = ArenaContextFrame::Keyed { data, index, key };
        }
    }

    #[inline]
    fn replace_reduce_data(&mut self, current: &'a DataValue<'a>, accumulator: &'a DataValue<'a>) {
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

    #[cfg(any(test, feature = "error-handling"))]
    #[inline]
    pub(crate) fn error_path_len(&self) -> usize {
        self.error_path.len()
    }

    #[cfg(any(test, feature = "error-handling"))]
    #[inline]
    pub(crate) fn truncate_error_path(&mut self, len: usize) {
        self.error_path.truncate(len);
    }

    #[cfg(feature = "compat")]
    #[inline]
    pub(crate) fn take_error_path(&mut self) -> Vec<u32> {
        std::mem::take(&mut self.error_path)
    }
}

// ---------------------------------------------------------------------------
// IterGuard
// ---------------------------------------------------------------------------

/// RAII guard for the per-iteration push/replace/pop pattern used by array
/// operators (filter / map / reduce / quantifiers / sort).
///
/// On the first `step_*` call the guard pushes a frame; subsequent `step_*`
/// calls *replace* the top frame in place (avoiding repeated push/pop). The
/// frame is popped automatically on drop, including on the early-return paths
/// that previously needed a manual `if pushed { actx.pop() }` epilogue.
///
/// All three iteration shapes are covered: indexed (array), keyed (object),
/// and reduce (current/accumulator).
pub(crate) struct IterGuard<'g, 'a> {
    actx: &'g mut DataContextStack<'a>,
    pushed: bool,
}

impl<'g, 'a> IterGuard<'g, 'a> {
    #[inline]
    pub(crate) fn new(actx: &'g mut DataContextStack<'a>) -> Self {
        Self {
            actx,
            pushed: false,
        }
    }

    #[inline]
    pub(crate) fn step_indexed(&mut self, data: &'a DataValue<'a>, index: usize) {
        if self.pushed {
            self.actx.replace_top_data(data, index);
        } else {
            self.actx.push_with_index(data, index);
            self.pushed = true;
        }
    }

    #[inline]
    pub(crate) fn step_keyed(&mut self, data: &'a DataValue<'a>, index: usize, key: &'a str) {
        if self.pushed {
            self.actx.replace_top_key_data(data, index, key);
        } else {
            self.actx.push_with_key_index(data, index, key);
            self.pushed = true;
        }
    }

    #[inline]
    pub(crate) fn step_reduce(
        &mut self,
        current: &'a DataValue<'a>,
        accumulator: &'a DataValue<'a>,
    ) {
        if self.pushed {
            self.actx.replace_reduce_data(current, accumulator);
        } else {
            self.actx.push_reduce(current, accumulator);
            self.pushed = true;
        }
    }

    /// Mutable access to the wrapped stack — for `engine.eval_iter_body(...)`
    /// and similar calls that take `&mut DataContextStack`.
    #[inline]
    pub(crate) fn stack(&mut self) -> &mut DataContextStack<'a> {
        self.actx
    }
}

impl Drop for IterGuard<'_, '_> {
    #[inline]
    fn drop(&mut self) {
        if self.pushed {
            self.actx.pop();
        }
    }
}

#[cfg(all(test, feature = "compat"))]
mod tests {
    use super::*;
    use crate::arena::value::DataValue;
    use serde_json::Value;

    #[test]
    fn lifecycle_indexed() {
        let arena = Bump::new();
        let root_val = Value::Null;
        let mut ctx = DataContextStack::from_value(&root_val, &arena);
        assert_eq!(ctx.depth(), 0);
        assert!(ctx.current().root_data().is_some(), "root at depth 0");

        let a: &DataValue = arena.alloc(DataValue::Number(crate::value::NumberValue::from_i64(1)));
        ctx.push_with_index(a, 0);
        assert_eq!(ctx.depth(), 1);
        assert_eq!(ctx.current().get_index(), Some(0));

        let b: &DataValue = arena.alloc(DataValue::Number(crate::value::NumberValue::from_i64(2)));
        ctx.replace_top_data(b, 1);
        assert_eq!(ctx.current().get_index(), Some(1));

        ctx.pop();
        assert_eq!(ctx.depth(), 0);
    }

    #[test]
    fn lifecycle_keyed() {
        let arena = Bump::new();
        let root_val = Value::Null;
        let mut ctx = DataContextStack::from_value(&root_val, &arena);

        let a: &DataValue = arena.alloc(DataValue::Bool(true));
        ctx.push_with_key_index(a, 0, "k1");
        assert_eq!(ctx.current().get_key(), Some("k1"));

        let b: &DataValue = arena.alloc(DataValue::Bool(false));
        ctx.replace_top_key_data(b, 1, "k2");
        assert_eq!(ctx.current().get_key(), Some("k2"));
        assert_eq!(ctx.current().get_index(), Some(1));
    }

    #[test]
    fn lifecycle_reduce() {
        let arena = Bump::new();
        let root_val = Value::Null;
        let mut ctx = DataContextStack::from_value(&root_val, &arena);

        let cur: &DataValue =
            arena.alloc(DataValue::Number(crate::value::NumberValue::from_i64(1)));
        let acc: &DataValue =
            arena.alloc(DataValue::Number(crate::value::NumberValue::from_i64(0)));
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
        let mut ctx = DataContextStack::from_value(&root_val, &arena);

        let a: &DataValue = arena.alloc(DataValue::Number(crate::value::NumberValue::from_i64(10)));
        let b: &DataValue = arena.alloc(DataValue::Number(crate::value::NumberValue::from_i64(20)));
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
    fn iter_guard_pushes_then_pops_indexed() {
        let arena = Bump::new();
        let root_val = Value::Null;
        let mut ctx = DataContextStack::from_value(&root_val, &arena);
        assert_eq!(ctx.depth(), 0);

        let a: &DataValue = arena.alloc(DataValue::Number(crate::value::NumberValue::from_i64(1)));
        let b: &DataValue = arena.alloc(DataValue::Number(crate::value::NumberValue::from_i64(2)));

        {
            let mut g = IterGuard::new(&mut ctx);
            g.step_indexed(a, 0);
            assert_eq!(g.stack().depth(), 1);
            assert_eq!(g.stack().current().get_index(), Some(0));
            g.step_indexed(b, 1);
            assert_eq!(g.stack().depth(), 1, "replace, not push");
            assert_eq!(g.stack().current().get_index(), Some(1));
        }
        assert_eq!(ctx.depth(), 0, "drop pops");
    }

    #[test]
    fn iter_guard_no_push_no_pop() {
        let arena = Bump::new();
        let root_val = Value::Null;
        let mut ctx = DataContextStack::from_value(&root_val, &arena);
        assert_eq!(ctx.depth(), 0);
        {
            let _g = IterGuard::new(&mut ctx);
            // empty input, no step_* calls
        }
        assert_eq!(ctx.depth(), 0, "drop without push is a no-op");
    }

    #[test]
    fn iter_guard_keyed_and_reduce() {
        let arena = Bump::new();
        let root_val = Value::Null;
        let mut ctx = DataContextStack::from_value(&root_val, &arena);

        let a: &DataValue = arena.alloc(DataValue::Bool(true));
        let b: &DataValue = arena.alloc(DataValue::Bool(false));

        {
            let mut g = IterGuard::new(&mut ctx);
            g.step_keyed(a, 0, "k1");
            assert_eq!(g.stack().current().get_key(), Some("k1"));
            g.step_keyed(b, 1, "k2");
            assert_eq!(g.stack().current().get_key(), Some("k2"));
            assert_eq!(g.stack().current().get_index(), Some(1));
        }
        assert_eq!(ctx.depth(), 0);

        let cur: &DataValue =
            arena.alloc(DataValue::Number(crate::value::NumberValue::from_i64(1)));
        let acc: &DataValue =
            arena.alloc(DataValue::Number(crate::value::NumberValue::from_i64(0)));
        {
            let mut g = IterGuard::new(&mut ctx);
            g.step_reduce(cur, acc);
            assert_eq!(g.stack().depth(), 1);
            g.step_reduce(acc, cur); // replace, not push
            assert_eq!(g.stack().depth(), 1);
        }
        assert_eq!(ctx.depth(), 0);
    }

    #[test]
    fn error_path_round_trip() {
        let arena = Bump::new();
        let root_val = Value::Null;
        let mut ctx = DataContextStack::from_value(&root_val, &arena);

        ctx.push_error_step(1);
        ctx.push_error_step(2);
        ctx.push_error_step(3);
        assert_eq!(ctx.error_path_len(), 3);

        ctx.truncate_error_path(1);
        let p = ctx.take_error_path();
        assert_eq!(p, vec![1]);
    }
}
