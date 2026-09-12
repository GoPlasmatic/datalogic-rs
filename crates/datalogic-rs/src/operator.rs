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

    /// Charge `n` operations against this evaluation's budget.
    ///
    /// Call this **before** doing work whose size the node count does not
    /// reflect — walking a large input, building a large result — so an
    /// over-budget rule is refused rather than run and then reported.
    /// `n` is in whatever unit makes the operator's cost proportional to
    /// its data: elements touched is the usual choice, and is what the
    /// built-in tensor family uses.
    ///
    /// The dispatcher already charges 1 for the operator node itself, so
    /// an operator whose work is bounded by a constant needs no charge at
    /// all.
    ///
    /// Always available. With the `budget` feature off it compiles to
    /// `Ok(())`, so an operator can call it unconditionally rather than
    /// carrying a `cfg` of its own.
    ///
    /// # Errors
    ///
    // `ErrorKind::BudgetExceeded` is gated behind `budget`; link it when
    // the feature is on, otherwise reference it as code text so the docs
    // stay resolvable in a default-features build.
    #[cfg_attr(
        feature = "budget",
        doc = "[`crate::ErrorKind::BudgetExceeded`] once the running total"
    )]
    #[cfg_attr(
        not(feature = "budget"),
        doc = "`ErrorKind::BudgetExceeded` (with the `budget` feature) once the running total"
    )]
    /// crosses the evaluation's ceiling. Propagate it — the counter stays
    /// exhausted, so there is nothing useful to do but unwind.
    ///
    /// ```rust
    /// use datalogic_rs::{CustomOperator, DataValue, Result, operator::EvalContext};
    ///
    /// struct Repeat;
    ///
    /// impl CustomOperator for Repeat {
    ///     fn evaluate<'a>(
    ///         &self,
    ///         args: &[&'a DataValue<'a>],
    ///         ctx: &mut EvalContext<'_, 'a>,
    ///         arena: &'a bumpalo::Bump,
    ///     ) -> Result<&'a DataValue<'a>> {
    ///         let text = args.first().and_then(|v| v.as_str()).unwrap_or("");
    ///         let times = args.get(1).and_then(|v| v.as_i64()).unwrap_or(0).max(0) as usize;
    ///         // Price the output before allocating it.
    ///         ctx.charge((text.len() * times) as u64)?;
    ///         Ok(arena.alloc(DataValue::String(arena.alloc_str(&text.repeat(times)))))
    ///     }
    /// }
    /// ```
    #[inline]
    pub fn charge(&mut self, n: u64) -> crate::Result<()> {
        self.inner.charge(n)
    }

    /// Engine-internal constructor. Used by the dispatcher when invoking a
    /// custom operator's `evaluate` method.
    #[inline]
    pub(crate) fn new(inner: &'ctx mut crate::arena::ContextStack<'a>) -> Self {
        Self { inner }
    }
}
