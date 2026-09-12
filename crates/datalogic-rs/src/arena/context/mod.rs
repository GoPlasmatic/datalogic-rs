//! `ContextStack` — context stack used during arena-mode evaluation.
//!
//! Frames hold `&'a DataValue<'a>`, and so does the root: callers either
//! pass an arena-resident value directly (e.g. `Engine::evaluate`) or use
//! `from_value` to deep-convert a borrowed `&Value` into the arena.
//!
//! Per-iteration cost: pushing a frame writes two pointers (no
//! `Value::clone`, no `BTreeMap::clone`). The current frame lives inline in
//! the struct (`top`), so the per-iteration replace/lookup accessors touch
//! struct-local memory only. Ancestor frames go into `parents`, which is
//! maintained only for the rare rule that can actually read one — see the
//! field's documentation.
//!
//! Submodules split the file by concern:
//! - [`frame`] — `ContextFrame`, the per-iteration payload.
//! - [`reference`] — `ContextRef`, the shared "frame or root" reference.
//!
//! `ContextStack` itself stays here alongside `IterGuard`, since the guard
//! mutates the stack's private frame storage directly.

mod frame;
mod reference;

pub(crate) use frame::ContextFrame;
pub(crate) use reference::ContextRef;

use super::value::DataValue;
#[cfg(all(test, feature = "serde_json"))]
use bumpalo::Bump;
use smallvec::SmallVec;

/// Arena-mode context stack. The lifetime `'a` is the arena lifetime; the
/// root is `&'a DataValue<'a>` (deep-converted from `&Value` for the public
/// API, or supplied directly by arena-native callers).
///
/// Frame storage is split into `top` (the current frame, inline in the
/// struct) and `parents` (everything below it, oldest first). The hot
/// per-iteration operations (`current`, `replace_*`) only ever touch
/// `top`; `parents` is touched on depth *transitions* (push/pop of nested
/// iterators) and level-walking lookups, both of which are rare. This
/// keeps the accessors free of the spill-check branch a plain `SmallVec`
/// frame stack would pay per lookup.
pub(crate) struct ContextStack<'a> {
    root: &'a DataValue<'a>,
    top: Option<ContextFrame<'a>>,
    /// Ancestor frames, oldest first — everything below `top`.
    ///
    /// Maintained **only** when the compiled rule can read an ancestor frame
    /// (`Logic::needs_ancestor_frames`). `get_at_level` reaches this list
    /// solely for `levels_up >= 2`, and restoring the enclosing frame now
    /// goes through [`FrameToken`], so for the overwhelming majority of rules
    /// nothing ever touches it — an ancestor is not even addressable until
    /// three levels of iterator nesting. A plain `Vec` rather than a
    /// `SmallVec`: an inline buffer would be paid for on every evaluation to
    /// serve well under 1% of them, whereas an unused `Vec` never allocates.
    parents: Vec<ContextFrame<'a>>,
    /// Whether to record ancestors at all — see `parents`.
    track_ancestors: bool,
    /// Live frame count, maintained explicitly rather than derived from
    /// `parents.len() + top.is_some()`. Keeping it independent is what lets
    /// `parents` stop being populated for rules that never read an ancestor
    /// frame — the count still has to be exact, because `get_at_level`'s
    /// clamp and the public `EvalContext::depth()` both read it.
    depth: u32,
    /// Breadcrumb of `CompiledNode::id`s accumulated as errors unwind.
    error_path: Vec<u32>,
    /// Per-evaluation CSE memo slots, indexed by `CseData::slot`. Fully
    /// lazy: starts empty and grows on the first [`Self::fill_cse_slot`],
    /// so evaluations of rules without CSE nodes never touch it (eager
    /// per-evaluation sizing cost a measurable regression on the
    /// folded-literal path; a heap-backed `Vec` cost CSE'd rules their
    /// win on small data). ≤ 8 slots stay inline. Reads past the current
    /// length are simply misses. `Ok`-results only; errors are never
    /// cached.
    cse_slots: SmallVec<[Option<&'a DataValue<'a>>; 8]>,
    /// Depth of enclosing `try` *protected* arms (every arm of a multi-arg
    /// `try` except the final catch arm). While > 0, any error raised is
    /// guaranteed to be consumed by the nearest enclosing `try`'s arm loop
    /// before it can reach a public boundary — which unlocks the deferred
    /// thrown-payload fast lane (`thrown_slot`).
    #[cfg(feature = "error-handling")]
    catch_depth: u32,
    /// Deferred thrown-payload channel: the arena-resident error object of
    /// an in-flight `Thrown` error raised inside a protected `try` arm.
    /// Written by the `throw` / NaN fast lanes together with a
    /// placeholder-payload `Error` (see `Error::deferred_thrown`); consumed
    /// by `try`'s catch arm, which pushes it as the error context without
    /// round-tripping through the owned payload. `try` clears it before
    /// each protected arm so a stale payload can never pair with an
    /// unrelated `Thrown` error from a non-deferring producer.
    #[cfg(feature = "error-handling")]
    thrown_slot: Option<&'a DataValue<'a>>,
    /// Operations charged so far this evaluation — see [`Self::charge`].
    /// Saturating, so a runaway rule pins the counter at `u64::MAX`
    /// instead of wrapping back under `budget`.
    #[cfg(feature = "budget")]
    ops: u64,
    /// Ceiling `ops` may not cross. `u64::MAX` when no budget is set,
    /// which makes the per-charge compare a never-taken branch rather
    /// than a second condition to test.
    #[cfg(feature = "budget")]
    budget: u64,
    /// Optional trace collector, owned by this stack while a traced
    /// evaluation is in flight. The trace driver moves a fresh collector
    /// in via [`Self::attach_tracer`] before dispatch and pulls it back out
    /// via [`Self::detach_tracer`] after. Owning (rather than borrowing)
    /// avoids tying the tracer's lifetime to `'a`, which is constrained by
    /// the arena reference and so can't accommodate a function-local
    /// collector. Tracing is a dev-time debugging feature, so the move
    /// cost is irrelevant.
    ///
    /// Boxed: the collector is 56 bytes and every evaluation in a
    /// trace-enabled build pays that as stack traffic whether or not it
    /// traces, while the one allocation lands only on the traced path
    /// where it is lost in the noise of rendering steps.
    #[cfg(feature = "trace")]
    tracer: Option<Box<crate::trace::TraceCollector>>,
}

/// Receipt for a pushed context frame: it carries the frame that was
/// displaced, so the pusher can put it back without the stack having to keep
/// an indexable ancestor list for the purpose.
///
/// `#[must_use]` is the point. Frame push/pop pairs used to be balanced by
/// hand, and an unbalanced one is a silent context corruption rather than a
/// crash — see `tests/error_context_test.rs`, which regression-tests exactly
/// that bug on the `map` bridge path. Now dropping the receipt without
/// restoring is a compile-time warning.
#[must_use = "hand the FrameToken back to `restore_frame`, or the displaced frame is lost"]
pub(crate) struct FrameToken<'a>(Option<ContextFrame<'a>>);

impl<'a> ContextStack<'a> {
    #[inline]
    pub(crate) fn new(root: &'a DataValue<'a>, track_ancestors: bool) -> Self {
        Self {
            root,
            top: None,
            parents: Vec::new(),
            track_ancestors,
            depth: 0,
            error_path: Vec::new(),
            cse_slots: SmallVec::new(),
            #[cfg(feature = "error-handling")]
            catch_depth: 0,
            #[cfg(feature = "error-handling")]
            thrown_slot: None,
            #[cfg(feature = "budget")]
            ops: 0,
            #[cfg(feature = "budget")]
            budget: u64::MAX,
            #[cfg(feature = "trace")]
            tracer: None,
        }
    }

    /// Build a context stack from a borrowed `&serde_json::Value` by
    /// deep-converting it into an arena-resident `DataValue`. Used only by
    /// the test module below — production v5 / compat paths construct a
    /// [`ContextStack::new`] directly with an arena-resident value.
    #[cfg(all(test, feature = "serde_json"))]
    #[inline]
    pub(crate) fn from_value(root: &'a serde_json::Value, arena: &'a Bump) -> Self {
        let av = crate::arena::value::value_to_data(root, arena);
        Self::new(arena.alloc(av), true)
    }

    /// Move a tracer into this stack. The trace driver pulls it back out
    /// via [`Self::detach_tracer`] after dispatch completes.
    #[cfg(feature = "trace")]
    #[inline]
    pub(crate) fn attach_tracer(&mut self, tracer: crate::trace::TraceCollector) {
        self.tracer = Some(Box::new(tracer));
    }

    /// Pull the tracer back out (e.g., after a traced evaluation completes)
    /// so the driver can extract the collected steps.
    #[cfg(feature = "trace")]
    #[inline]
    pub(crate) fn detach_tracer(&mut self) -> Option<crate::trace::TraceCollector> {
        self.tracer.take().map(|boxed| *boxed)
    }

    /// True iff a tracer has been attached.
    #[cfg(feature = "trace")]
    #[inline]
    pub(crate) fn has_tracer(&self) -> bool {
        self.tracer.is_some()
    }

    /// Run `f` against the attached tracer. No-op if no tracer is set.
    #[cfg(feature = "trace")]
    #[inline]
    fn with_tracer<F: FnOnce(&mut crate::trace::TraceCollector)>(&mut self, f: F) {
        if let Some(tracer) = self.tracer.as_mut() {
            f(tracer);
        }
    }

    /// Cross-feature wrapper around [`has_tracer`]. Always callable; folds to
    /// `false` when the `trace` feature is off so callers don't need their own
    /// `cfg` shims. Used by iterator-op fast paths to skip optimizations that
    /// would bypass [`run_iter_body`]'s trace markers.
    #[inline]
    pub(crate) fn is_tracing(&self) -> bool {
        #[cfg(feature = "trace")]
        {
            self.has_tracer()
        }
        #[cfg(not(feature = "trace"))]
        {
            false
        }
    }

    /// Snapshot the current frame's data as an owned `Value`. Used by the
    /// arena dispatcher before recursing into a child, so the trace step
    /// can record the context that operator saw.
    #[cfg(all(feature = "trace", feature = "serde_json"))]
    pub(crate) fn current_data_as_value(&self) -> serde_json::Value {
        crate::arena::data_to_value(self.current().data())
    }

    /// Record the result of a node into the attached tracer. No-op if no
    /// tracer is attached. Callers gate on [`has_tracer`] first to skip the
    /// `Value::clone()` when not tracing.
    #[cfg(all(feature = "trace", feature = "serde_json"))]
    pub(crate) fn record_node_result(
        &mut self,
        node_id: u32,
        ctx_data: serde_json::Value,
        result: &crate::Result<&'a crate::arena::DataValue<'a>>,
    ) {
        self.with_tracer(|collector| match result {
            Ok(av) => {
                let v = crate::arena::data_to_value(av);
                collector.record_step(node_id, ctx_data, v);
            }
            Err(e) => {
                collector.record_error(node_id, ctx_data, e.to_string());
            }
        });
    }

    /// Mark entry into an iteration body — drives the per-step
    /// `iteration_index` / `iteration_total` fields on traced steps.
    #[cfg(feature = "trace")]
    #[inline]
    pub(crate) fn trace_push_iteration(&mut self, index: u32, total: u32) {
        self.with_tracer(|c| c.push_iteration(index, total));
    }

    /// Mark exit from an iteration body.
    #[cfg(feature = "trace")]
    #[inline]
    pub(crate) fn trace_pop_iteration(&mut self) {
        self.with_tracer(|c| c.pop_iteration());
    }

    /// Get the root input data (borrowed for the call's duration).
    #[inline]
    pub(crate) fn root_input(&self) -> &'a DataValue<'a> {
        self.root
    }

    /// Current depth (number of pushed iteration frames).
    #[inline]
    pub(crate) fn depth(&self) -> usize {
        debug_assert!(
            !self.track_ancestors
                || self.depth as usize == self.parents.len() + usize::from(self.top.is_some()),
            "explicit depth counter drifted from the frame storage"
        );
        self.depth as usize
    }

    // ----- CSE memo slots ---------------------------------------------------

    /// Read a CSE memo slot. `None` for a miss — including slots the lazy
    /// table hasn't grown to yet, so no per-evaluation setup is needed
    /// (rules without CSE nodes never touch the table at all).
    #[inline]
    pub(crate) fn cse_slot(&self, slot: u16) -> Option<&'a DataValue<'a>> {
        self.cse_slots.get(slot as usize).copied().flatten()
    }

    /// Fill a CSE memo slot with an `Ok` result, lazily growing the table
    /// on first use. Slot indices come from `Logic`'s compile-time
    /// numbering, so the table never exceeds `Logic::cse_slot_count`
    /// entries.
    #[inline]
    pub(crate) fn fill_cse_slot(&mut self, slot: u16, value: &'a DataValue<'a>) {
        let index = slot as usize;
        if index >= self.cse_slots.len() {
            self.cse_slots.resize(index + 1, None);
        }
        self.cse_slots[index] = Some(value);
    }

    /// Get the current context (top frame, or root if empty).
    #[inline]
    pub(crate) fn current(&self) -> ContextRef<'a, '_> {
        if let Some(frame) = self.top.as_ref() {
            ContextRef::Frame(frame)
        } else {
            ContextRef::Root(self.root)
        }
    }

    /// Walk `level` frames up from the current context. Negative/positive
    /// magnitudes treated as absolute (matches `ContextStack::get_at_level`).
    /// Index arithmetic is unchanged from the single-`Vec` layout: the
    /// conceptual frame list is `parents ++ [top]`, so `parents.len()` is
    /// the top frame's index.
    pub(crate) fn get_at_level(&self, level: isize) -> Option<ContextRef<'a, '_>> {
        let levels_up = level.unsigned_abs();
        if levels_up == 0 {
            return Some(self.current());
        }
        let frame_count = self.depth();
        if levels_up >= frame_count {
            return Some(ContextRef::Root(self.root));
        }
        let target_index = frame_count - levels_up;
        // `levels_up == 1` names the top frame itself; anything deeper is an
        // ancestor, which is the one case that needs the list.
        if levels_up == 1 {
            return self.top.as_ref().map(ContextRef::Frame);
        }
        debug_assert!(
            self.track_ancestors,
            "ancestor lookup on a stack built without ancestor tracking - \
             `Logic::needs_ancestor_frames` under-approximated"
        );
        self.parents.get(target_index).map(ContextRef::Frame)
    }

    // ----- frame mutation ---------------------------------------------------

    /// Push a frame, returning the displaced one as a [`FrameToken`] for the
    /// caller to restore. `ContextFrame` is `Copy`, so the token is a cheap
    /// duplicate of what also went into `parents`.
    #[inline]
    fn push_frame(&mut self, frame: ContextFrame<'a>) -> FrameToken<'a> {
        let prev = self.top.replace(frame);
        if self.track_ancestors
            && let Some(p) = prev
        {
            self.parents.push(p);
        }
        self.depth += 1;
        FrameToken(prev)
    }

    #[inline]
    pub(crate) fn push(&mut self, data: &'a DataValue<'a>) -> FrameToken<'a> {
        self.push_frame(ContextFrame::Data(data))
    }

    /// Push an indexed frame. Used by `IterGuard` and by `map`'s scalar
    /// bridge path, which pushes a single `index: 0` frame directly.
    #[inline]
    pub(crate) fn push_indexed(&mut self, data: &'a DataValue<'a>, index: usize) -> FrameToken<'a> {
        self.push_frame(ContextFrame::Indexed { data, index })
    }

    #[inline]
    fn push_with_key_index(
        &mut self,
        data: &'a DataValue<'a>,
        index: usize,
        key: &'a str,
    ) -> FrameToken<'a> {
        self.push_frame(ContextFrame::Keyed { data, index, key })
    }

    #[inline]
    fn push_reduce(
        &mut self,
        current: &'a DataValue<'a>,
        accumulator: &'a DataValue<'a>,
    ) -> FrameToken<'a> {
        self.push_frame(ContextFrame::Reduce {
            current,
            accumulator,
        })
    }

    #[inline]
    fn replace_top_data(&mut self, data: &'a DataValue<'a>, index: usize) {
        if let Some(frame) = self.top.as_mut() {
            *frame = ContextFrame::Indexed { data, index };
        }
    }

    #[inline]
    fn replace_top_key_data(&mut self, data: &'a DataValue<'a>, index: usize, key: &'a str) {
        if let Some(frame) = self.top.as_mut() {
            *frame = ContextFrame::Keyed { data, index, key };
        }
    }

    #[inline]
    fn replace_reduce_data(&mut self, current: &'a DataValue<'a>, accumulator: &'a DataValue<'a>) {
        if let Some(frame) = self.top.as_mut() {
            *frame = ContextFrame::Reduce {
                current,
                accumulator,
            };
        }
    }

    /// Undo a [`push_frame`](Self::push_frame), restoring the frame the token
    /// carries. The restored value comes from the token rather than from
    /// `parents`, which is what frees `parents` to be maintained only when a
    /// rule actually reads an ancestor frame.
    #[inline]
    pub(crate) fn restore_frame(&mut self, token: FrameToken<'a>) {
        self.top = token.0;
        if self.track_ancestors && token.0.is_some() {
            self.parents.pop();
        }
        self.depth -= 1;
    }

    // ----- error breadcrumb (mirrors ContextStack) --------------------------

    #[cold]
    #[inline(never)]
    pub(crate) fn push_error_step(&mut self, id: u32) {
        self.error_path.push(id);
    }

    #[cfg(feature = "error-handling")]
    #[inline]
    pub(crate) fn error_path_len(&self) -> usize {
        self.error_path.len()
    }

    #[cfg(feature = "error-handling")]
    #[inline]
    pub(crate) fn truncate_error_path(&mut self, len: usize) {
        self.error_path.truncate(len);
    }

    /// Move the breadcrumb out of the stack, leaving an empty `Vec` behind.
    /// Used by the public `evaluate*` methods to attach the path to the
    /// returned [`crate::Error`] on failure.
    #[inline]
    pub(crate) fn take_error_path(&mut self) -> Vec<u32> {
        std::mem::take(&mut self.error_path)
    }

    // ----- operation budget --------------------------------------------------

    /// Charge `n` operations against this evaluation's budget.
    ///
    /// Every charge happens **before** the work it pays for, so a rule
    /// that would build a billion-element result is refused rather than
    /// run and then reported. The dispatcher charges 1 per node,
    /// iterator operators charge 1 per item they are about to examine,
    /// and operators that move an amount of data the node count does not
    /// reflect (the tensor family) charge their own element count.
    ///
    /// Compiles to `Ok(())` — no counter, no compare — when the `budget`
    /// feature is off.
    ///
    /// # Errors
    ///
    /// [`crate::ErrorKind::BudgetExceeded`] once the running total
    /// crosses the ceiling. The counter is left past the ceiling, so
    /// every later charge fails too: that is what makes the abort final
    /// rather than something a `try` arm can step over.
    #[cfg(feature = "budget")]
    #[inline]
    pub(crate) fn charge(&mut self, n: u64) -> crate::Result<()> {
        self.ops = self.ops.saturating_add(n);
        if self.ops > self.budget {
            return Err(crate::Error::budget_exceeded(self.budget, self.ops));
        }
        Ok(())
    }

    /// Cross-feature no-op form of [`Self::charge`].
    #[cfg(not(feature = "budget"))]
    #[inline(always)]
    pub(crate) fn charge(&mut self, _n: u64) -> crate::Result<()> {
        Ok(())
    }

    /// Set the ceiling for this evaluation. `u64::MAX` means unbounded.
    #[cfg(feature = "budget")]
    #[inline]
    pub(crate) fn set_budget(&mut self, budget: u64) {
        self.budget = budget;
    }

    /// Operations charged so far.
    #[cfg(feature = "budget")]
    #[inline]
    pub(crate) fn ops_spent(&self) -> u64 {
        self.ops
    }

    // ----- deferred thrown-payload channel (see field docs) ------------------

    /// True while evaluation is inside a protected (non-final) arm of a
    /// multi-arg `try` — i.e. any error raised now is guaranteed to be
    /// caught by the enclosing `try`'s arm loop.
    #[cfg(feature = "error-handling")]
    #[inline]
    pub(crate) fn in_catch_scope(&self) -> bool {
        self.catch_depth > 0
    }

    /// Enter a protected `try` arm. Must be paired with
    /// [`Self::exit_catch_scope`] on every path out of the arm.
    #[cfg(feature = "error-handling")]
    #[inline]
    pub(crate) fn enter_catch_scope(&mut self) {
        self.catch_depth += 1;
    }

    /// Leave a protected `try` arm.
    #[cfg(feature = "error-handling")]
    #[inline]
    pub(crate) fn exit_catch_scope(&mut self) {
        debug_assert!(self.catch_depth > 0, "unbalanced exit_catch_scope");
        self.catch_depth -= 1;
    }

    /// Park the arena-form payload of an in-flight deferred `Thrown` error.
    /// Only call together with constructing `Error::deferred_thrown()` while
    /// [`Self::in_catch_scope`] is true.
    #[cfg(feature = "error-handling")]
    #[inline]
    pub(crate) fn set_thrown_slot(&mut self, payload: &'a DataValue<'a>) {
        self.thrown_slot = Some(payload);
    }

    /// Consume the deferred thrown payload, if any.
    #[cfg(feature = "error-handling")]
    #[inline]
    pub(crate) fn take_thrown_slot(&mut self) -> Option<&'a DataValue<'a>> {
        self.thrown_slot.take()
    }

    /// Drop any deferred thrown payload. `try` calls this before each
    /// protected arm so a stale payload from an earlier arm (or an
    /// enclosing `try`) can't pair with an unrelated `Thrown` error raised
    /// through a non-deferring site.
    #[cfg(feature = "error-handling")]
    #[inline]
    pub(crate) fn clear_thrown_slot(&mut self) {
        self.thrown_slot = None;
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
/// that previously needed a manual `if pushed { ctx.pop() }` epilogue.
///
/// All three iteration shapes are covered: indexed (array), keyed (object),
/// and reduce (current/accumulator).
pub(crate) struct IterGuard<'g, 'a> {
    ctx: &'g mut ContextStack<'a>,
    /// `Some` once a frame has been pushed; carries what to restore on drop.
    saved: Option<FrameToken<'a>>,
}

impl<'g, 'a> IterGuard<'g, 'a> {
    #[inline]
    pub(crate) fn new(ctx: &'g mut ContextStack<'a>) -> Self {
        Self { ctx, saved: None }
    }

    #[inline]
    pub(crate) fn step_indexed(&mut self, data: &'a DataValue<'a>, index: usize) {
        if self.saved.is_some() {
            self.ctx.replace_top_data(data, index);
        } else {
            self.saved = Some(self.ctx.push_indexed(data, index));
        }
    }

    #[inline]
    pub(crate) fn step_keyed(&mut self, data: &'a DataValue<'a>, index: usize, key: &'a str) {
        if self.saved.is_some() {
            self.ctx.replace_top_key_data(data, index, key);
        } else {
            self.saved = Some(self.ctx.push_with_key_index(data, index, key));
        }
    }

    #[inline]
    pub(crate) fn step_reduce(
        &mut self,
        current: &'a DataValue<'a>,
        accumulator: &'a DataValue<'a>,
    ) {
        if self.saved.is_some() {
            self.ctx.replace_reduce_data(current, accumulator);
        } else {
            self.saved = Some(self.ctx.push_reduce(current, accumulator));
        }
    }

    /// Mutable access to the wrapped stack — for `engine.run_iter_body(...)`
    /// and similar calls that take `&mut ContextStack`.
    #[inline]
    pub(crate) fn stack(&mut self) -> &mut ContextStack<'a> {
        self.ctx
    }
}

impl Drop for IterGuard<'_, '_> {
    #[inline]
    fn drop(&mut self) {
        if let Some(token) = self.saved.take() {
            self.ctx.restore_frame(token);
        }
    }
}

#[cfg(all(test, feature = "serde_json"))]
mod tests {
    use super::*;
    use crate::arena::value::DataValue;
    use serde_json::Value;

    #[test]
    fn lifecycle_indexed() {
        let arena = Bump::new();
        let root_val = Value::Null;
        let mut ctx = ContextStack::from_value(&root_val, &arena);
        assert_eq!(ctx.depth(), 0);
        assert!(ctx.current().root_data().is_some(), "root at depth 0");

        let a: &DataValue = arena.alloc(DataValue::Number(datavalue::NumberValue::from_i64(1)));
        let token = ctx.push_indexed(a, 0);
        assert_eq!(ctx.depth(), 1);
        assert_eq!(ctx.current().get_index(), Some(0));

        let b: &DataValue = arena.alloc(DataValue::Number(datavalue::NumberValue::from_i64(2)));
        ctx.replace_top_data(b, 1);
        assert_eq!(ctx.current().get_index(), Some(1));

        ctx.restore_frame(token);
        assert_eq!(ctx.depth(), 0);
    }

    #[test]
    fn lifecycle_keyed() {
        let arena = Bump::new();
        let root_val = Value::Null;
        let mut ctx = ContextStack::from_value(&root_val, &arena);

        let a: &DataValue = arena.alloc(DataValue::Bool(true));
        let _token = ctx.push_with_key_index(a, 0, "k1");
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
        let mut ctx = ContextStack::from_value(&root_val, &arena);

        let cur: &DataValue = arena.alloc(DataValue::Number(datavalue::NumberValue::from_i64(1)));
        let acc: &DataValue = arena.alloc(DataValue::Number(datavalue::NumberValue::from_i64(0)));
        let _token = ctx.push_reduce(cur, acc);
        assert_eq!(ctx.depth(), 1);

        if let ContextRef::Frame(f) = ctx.current() {
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
        let mut ctx = ContextStack::from_value(&root_val, &arena);

        let a: &DataValue = arena.alloc(DataValue::Number(datavalue::NumberValue::from_i64(10)));
        let b: &DataValue = arena.alloc(DataValue::Number(datavalue::NumberValue::from_i64(20)));
        let _ta = ctx.push_indexed(a, 0);
        let _tb = ctx.push_indexed(b, 0);
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
    fn pop_restores_parent_frames() {
        let arena = Bump::new();
        let root_val = Value::Null;
        let mut ctx = ContextStack::from_value(&root_val, &arena);

        let a: &DataValue = arena.alloc(DataValue::Number(datavalue::NumberValue::from_i64(1)));
        let b: &DataValue = arena.alloc(DataValue::Number(datavalue::NumberValue::from_i64(2)));
        let c: &DataValue = arena.alloc(DataValue::Number(datavalue::NumberValue::from_i64(3)));
        let ta = ctx.push_indexed(a, 10);
        let tb = ctx.push_indexed(b, 20);
        let tc = ctx.push_indexed(c, 30);
        assert_eq!(ctx.depth(), 3);
        assert_eq!(ctx.current().get_index(), Some(30));

        ctx.restore_frame(tc);
        assert_eq!(ctx.depth(), 2);
        assert_eq!(ctx.current().get_index(), Some(20), "parent restored");

        ctx.restore_frame(tb);
        assert_eq!(ctx.current().get_index(), Some(10));

        ctx.restore_frame(ta);
        assert_eq!(ctx.depth(), 0);
        assert!(ctx.current().root_data().is_some(), "back to root");
    }

    #[test]
    fn deep_nesting_spills_and_unwinds() {
        // Nest well past any plausible real rule, then verify level walking
        // and unwinding all the way back down.
        let arena = Bump::new();
        let root_val = Value::Null;
        let mut ctx = ContextStack::from_value(&root_val, &arena);

        let depth = 8;
        let mut tokens = Vec::new();
        for i in 0..depth {
            let v: &DataValue = arena.alloc(DataValue::Number(datavalue::NumberValue::from_i64(
                i as i64,
            )));
            tokens.push(ctx.push_indexed(v, i));
        }
        assert_eq!(ctx.depth(), depth);
        assert_eq!(ctx.current().get_index(), Some(depth - 1));

        // Walking `levels_up` lands on frame index `depth - levels_up`
        // (existing single-`Vec` arithmetic), and past the bottom is root.
        for levels_up in 1..depth {
            let r = ctx.get_at_level(levels_up as isize).expect("in range");
            assert_eq!(r.get_index(), Some(depth - levels_up));
        }
        assert!(
            ctx.get_at_level(depth as isize)
                .and_then(|r| r.root_data())
                .is_some()
        );

        for i in (0..depth).rev() {
            assert_eq!(ctx.current().get_index(), Some(i));
            ctx.restore_frame(tokens.pop().expect("one token per push"));
        }
        assert_eq!(ctx.depth(), 0);
        assert!(tokens.is_empty());
    }

    #[test]
    fn iter_guard_pushes_then_pops_indexed() {
        let arena = Bump::new();
        let root_val = Value::Null;
        let mut ctx = ContextStack::from_value(&root_val, &arena);
        assert_eq!(ctx.depth(), 0);

        let a: &DataValue = arena.alloc(DataValue::Number(datavalue::NumberValue::from_i64(1)));
        let b: &DataValue = arena.alloc(DataValue::Number(datavalue::NumberValue::from_i64(2)));

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
        let mut ctx = ContextStack::from_value(&root_val, &arena);
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
        let mut ctx = ContextStack::from_value(&root_val, &arena);

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

        let cur: &DataValue = arena.alloc(DataValue::Number(datavalue::NumberValue::from_i64(1)));
        let acc: &DataValue = arena.alloc(DataValue::Number(datavalue::NumberValue::from_i64(0)));
        {
            let mut g = IterGuard::new(&mut ctx);
            g.step_reduce(cur, acc);
            assert_eq!(g.stack().depth(), 1);
            g.step_reduce(acc, cur); // replace, not push
            assert_eq!(g.stack().depth(), 1);
        }
        assert_eq!(ctx.depth(), 0);
    }

    #[cfg(feature = "error-handling")]
    #[test]
    fn error_path_round_trip() {
        let arena = Bump::new();
        let root_val = Value::Null;
        let mut ctx = ContextStack::from_value(&root_val, &arena);

        ctx.push_error_step(1);
        ctx.push_error_step(2);
        ctx.push_error_step(3);
        assert_eq!(ctx.error_path_len(), 3);

        ctx.truncate_error_path(1);
        let p = ctx.take_error_path();
        assert_eq!(p, vec![1]);
    }

    #[cfg(feature = "error-handling")]
    #[test]
    fn thrown_slot_and_catch_scope_round_trip() {
        let arena = Bump::new();
        let root_val = Value::Null;
        let mut ctx = ContextStack::from_value(&root_val, &arena);

        assert!(!ctx.in_catch_scope());
        ctx.enter_catch_scope();
        ctx.enter_catch_scope();
        assert!(ctx.in_catch_scope());
        ctx.exit_catch_scope();
        assert!(ctx.in_catch_scope(), "nested scopes count");
        ctx.exit_catch_scope();
        assert!(!ctx.in_catch_scope());

        assert!(ctx.take_thrown_slot().is_none());
        let payload: &DataValue = arena.alloc(DataValue::Bool(true));
        ctx.set_thrown_slot(payload);
        assert!(std::ptr::eq(
            ctx.take_thrown_slot().expect("slot set"),
            payload
        ));
        assert!(ctx.take_thrown_slot().is_none(), "take consumes");

        ctx.set_thrown_slot(payload);
        ctx.clear_thrown_slot();
        assert!(ctx.take_thrown_slot().is_none(), "clear drops");
    }
    /// `ContextStack` is built fresh on every `Engine::evaluate`, so its size
    /// is per-evaluation stack traffic, not a one-off. It was 400 bytes when
    /// `parents` carried a four-slot inline buffer — storage that a census of
    /// the conformance corpus showed 99.1% of rules never write, because an
    /// ancestor frame is not addressable until three levels of iterator
    /// nesting. Dropping the buffer was worth 7-15% on shallow rules.
    ///
    /// Shrink the payload rather than raising this bound. The `budget`
    /// feature's two counters were paid for by boxing the trace
    /// collector, which no evaluation reads unless it is being traced.
    #[test]
    #[cfg(target_pointer_width = "64")]
    fn context_stack_stays_small() {
        let size = std::mem::size_of::<ContextStack<'_>>();
        assert!(
            size <= 256,
            "ContextStack grew to {size} bytes; it is constructed per evaluation"
        );
    }
}
