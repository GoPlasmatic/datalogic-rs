//! Public scaffolding for user-supplied operators.
//!
//! Custom operators implement [`crate::CustomOperator`] and receive an
//! [`EvalContext`] handle alongside the pre-evaluated arguments and arena.
//! The handle is opaque: it exposes the read-only context observations a
//! custom operator may legitimately need ([`EvalContext::root_input`],
//! [`EvalContext::depth`]) and hides the internal evaluation stack so its
//! layout can evolve without breaking the trait contract.

/// Opaque view into the engine's evaluation context, passed to
/// [`crate::CustomOperator::evaluate`].
///
/// `'a` is the arena lifetime — the same `'a` that scopes the borrowed
/// `&'a DataValue<'a>` arguments and the `&'a Bump` allocator. `'ctx`
/// scopes the underlying `&mut` borrow into the engine's stack and is
/// elided in user code (write `EvalContext<'_, 'a>` and Rust fills in the
/// outer lifetime).
///
/// Custom operators rarely need to inspect the context; the dominant
/// reason to take `ctx` at all is so the trait signature can grow new
/// observations in future 5.x releases without breaking existing impls.
/// The internals of this type are deliberately hidden behind the
/// accessors below so the layout can evolve without breaking the
/// [`crate::CustomOperator`] contract — see that trait's *Stability*
/// section for the full forward-compat commitment.
pub struct EvalContext<'ctx, 'a> {
    inner: &'ctx mut crate::arena::ContextStack<'a>,
}

impl<'ctx, 'a> EvalContext<'ctx, 'a> {
    /// The root input passed to [`crate::Engine::evaluate`]. Stable across
    /// the entire evaluation — does not change as iteration frames are
    /// pushed/popped by enclosing operators.
    #[inline]
    pub fn root_input(&self) -> &'a crate::DataValue<'a> {
        self.inner.root_input()
    }

    /// Number of iteration frames currently pushed by enclosing operators.
    /// Zero at the top level. Useful when an operator's behaviour depends
    /// on whether it's being invoked inside a `filter` / `map` / `reduce`.
    #[inline]
    pub fn depth(&self) -> usize {
        self.inner.depth()
    }

    /// Engine-internal constructor. Used by the dispatcher when invoking a
    /// custom operator's `evaluate` method.
    #[inline]
    pub(crate) fn new(inner: &'ctx mut crate::arena::ContextStack<'a>) -> Self {
        Self { inner }
    }
}
