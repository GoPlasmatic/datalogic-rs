//! Reusable evaluation handle that owns its arena.
//!
//! [`Session`] owns a [`bumpalo::Bump`] and exposes [`Session::reset`] for the
//! caller to bound peak memory between calls. The session itself never resets
//! the arena — every `evaluate*` method appends to the bump and the caller
//! decides when to release that memory back to the start-of-chunk position.
//! Inputs go through [`crate::EvalInput`] so callers pass whatever they have
//! on hand (`&str`, `&OwnedDataValue`, `&serde_json::Value`, …); outputs are
//! either owned ([`OwnedDataValue`] / `String` / `serde_json::Value`) or
//! borrowed from the arena ([`Self::evaluate_borrowed`]) — borrowed results are
//! invalidated by the next `&mut self` call (Rust's borrow checker enforces).
//!
//! For a one-shot evaluation that owns and drops its arena per call, use
//! [`crate::Engine::evaluate_str`] (convenience). For full caller control of
//! the arena lifecycle, use [`crate::Engine::evaluate`] directly with a
//! caller-passed `&Bump`.

use bumpalo::Bump;
use datavalue::OwnedDataValue;

use crate::arena::DataValue;
use crate::{Engine, EvalInput, Logic, Result};

/// Reusable evaluation handle. Construct via [`Engine::session`].
///
/// Owns a [`bumpalo::Bump`]; the caller controls reset via [`Self::reset`].
/// Subsequent `evaluate*` calls append to the bump until the caller resets
/// or the session is dropped — letting the caller amortise reset cost across
/// logical batches and avoid resetting between calls that don't need it.
///
/// # Example
///
/// ```rust
/// use datalogic_rs::Engine;
///
/// let engine = Engine::new();
/// let compiled = engine.compile(r#"{"+": [{"var": "x"}, 1]}"#).unwrap();
/// let mut session = engine.session();
///
/// for x in 0..3 {
///     let payload = format!(r#"{{"x": {}}}"#, x);
///     let result = session.evaluate_str(&compiled, &payload).unwrap();
///     assert_eq!(result, (x + 1).to_string());
///     // Reset between iterations to keep peak memory bounded by the
///     // largest single evaluation rather than the cumulative loop.
///     session.reset();
/// }
/// ```
pub struct Session<'engine> {
    engine: &'engine Engine,
    arena: Bump,
}

impl std::fmt::Debug for Session<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        // Print the engine handle plus the arena's currently-allocated byte
        // count — useful for tracking high-water marks across resets without
        // dumping every chunk's raw bytes.
        f.debug_struct("Session")
            .field("engine", &self.engine)
            .field("arena_allocated_bytes", &self.arena.allocated_bytes())
            .finish_non_exhaustive()
    }
}

impl<'engine> Session<'engine> {
    #[inline]
    pub(crate) fn new(engine: &'engine Engine) -> Self {
        Self {
            engine,
            arena: Bump::new(),
        }
    }

    /// Reset the session's arena, returning every allocated chunk to the
    /// free list's start-of-chunk position without freeing OS memory.
    ///
    /// Call this between logical batches to bound peak memory. After reset,
    /// any borrowed reference previously returned by [`Self::evaluate_borrowed`]
    /// is invalidated — the borrow checker enforces this for the common case
    /// (the result borrow ends with the previous `&mut self` borrow).
    ///
    /// `Bump::reset` is constant-time (resets a few pointers); the freed
    /// chunks remain allocated and serve subsequent calls without re-asking
    /// the OS for memory.
    #[inline]
    pub fn reset(&mut self) {
        self.arena.reset();
    }

    /// Drop the session's arena and replace it with a fresh one whose
    /// initial chunk holds at least `capacity` bytes.
    ///
    /// Use this when you know the steady-state high-water mark of your
    /// workload (e.g. captured via [`Self::allocated_bytes`] after a
    /// warm-up pass) and want subsequent calls to run on a single
    /// pre-sized chunk — no chunk-growth events during the timed window.
    ///
    /// Unlike [`Self::reset`], which keeps the existing chunks and only
    /// rewinds the bump pointer, this drops the chunks entirely and
    /// allocates one new chunk of the requested capacity. Any reference
    /// previously returned by [`Self::evaluate_borrowed`] is invalidated; the
    /// `&mut self` signature lets the borrow checker enforce this.
    pub fn reset_with_capacity(&mut self, capacity: usize) {
        self.arena = Bump::with_capacity(capacity);
    }

    /// Total bytes currently occupied by the session's arena chunks.
    ///
    /// Useful for capturing a workload's steady-state high-water mark
    /// after a warm-up pass — feed the returned value into
    /// [`Self::reset_with_capacity`] to pre-size the arena before a timed
    /// loop. Stable across [`Self::reset`] calls (chunks aren't freed);
    /// drops to the new chunk size after [`Self::reset_with_capacity`].
    ///
    /// Forwards to [`bumpalo::Bump::allocated_bytes`].
    #[inline]
    pub fn allocated_bytes(&self) -> usize {
        self.arena.allocated_bytes()
    }

    /// Evaluate `compiled` against `data` and deep-clone the result into an
    /// [`OwnedDataValue`] that survives subsequent calls and resets.
    ///
    /// The intermediate arena allocations stay in the session's bump until
    /// the caller invokes [`Self::reset`]. For long-running loops, call
    /// `reset` between iterations to keep peak memory bounded.
    ///
    /// # Example
    ///
    /// ```rust
    /// use datalogic_rs::Engine;
    ///
    /// let engine = Engine::new();
    /// let compiled = engine.compile(r#"{"==": [{"var": "x"}, 1]}"#).unwrap();
    /// let mut session = engine.session();
    /// let result = session.evaluate(&compiled, r#"{"x": 1}"#).unwrap();
    /// assert_eq!(result.as_bool(), Some(true));
    /// ```
    pub fn evaluate<'a, D>(&'a mut self, compiled: &Logic, data: D) -> Result<OwnedDataValue>
    where
        D: EvalInput<'a>,
    {
        let arena: &'a Bump = &self.arena;
        let av = data.into_arena_value(arena)?;
        let result = self.engine.evaluate(compiled, av, arena)?;
        Ok(result.to_owned())
    }

    /// Evaluate and return a borrowed result tied to this session's arena.
    ///
    /// Same semantics as [`Self::evaluate`] but skips the deep-clone into
    /// [`OwnedDataValue`] — the returned reference is invalidated by the
    /// next `&mut self` call (the borrow checker enforces).
    /// Use this when the result is consumed before the next session call; for
    /// cross-call retention, use [`Self::evaluate`].
    ///
    /// Symmetric with [`Engine::evaluate`] (caller-managed bump, borrowed
    /// result) but with the bump owned by the session. Like every other
    /// `evaluate*` method here, this does not reset the arena — call
    /// [`Self::reset`] explicitly to bound peak memory.
    ///
    /// # Example
    ///
    /// ```rust
    /// use datalogic_rs::Engine;
    ///
    /// let engine = Engine::new();
    /// let compiled = engine.compile(r#"{"+": [{"var": "x"}, 1]}"#).unwrap();
    /// let mut session = engine.session();
    /// let result = session.evaluate_borrowed(&compiled, r#"{"x": 5}"#).unwrap();
    /// assert_eq!(result.as_i64(), Some(6));
    /// ```
    pub fn evaluate_borrowed<'a, D>(
        &'a mut self,
        compiled: &'a Logic,
        data: D,
    ) -> Result<&'a DataValue<'a>>
    where
        D: EvalInput<'a>,
    {
        let arena: &'a Bump = &self.arena;
        let av = data.into_arena_value(arena)?;
        self.engine.evaluate(compiled, av, arena)
    }

    /// JSON-string convenience: evaluate `compiled` against a JSON-encoded
    /// `data` payload and serialise the result back to a JSON `String`.
    /// Mirrors [`Engine::evaluate_str`] but reuses the arena across calls.
    /// Does not reset the arena — see [`Self::reset`].
    pub fn evaluate_str(&mut self, compiled: &Logic, data: &str) -> Result<String> {
        let arena: &Bump = &self.arena;
        let av = data.into_arena_value(arena)?;
        let result = self.engine.evaluate(compiled, av, arena)?;
        Ok(crate::arena::data_to_json_string(result))
    }

    /// `serde_json::Value` convenience: evaluate `compiled` against a serde
    /// value and convert the result back to a serde value. Mirrors
    /// [`Engine::evaluate_json_value`] but reuses the arena across calls.
    /// Does not reset the arena — see [`Self::reset`].
    #[cfg(feature = "compat")]
    #[cfg_attr(docsrs, doc(cfg(feature = "compat")))]
    pub fn evaluate_json_value(
        &mut self,
        compiled: &Logic,
        data: &serde_json::Value,
    ) -> Result<serde_json::Value> {
        let arena: &Bump = &self.arena;
        let av = data.into_arena_value(arena)?;
        let result = self.engine.evaluate(compiled, av, arena)?;
        Ok(crate::arena::data_to_value(result))
    }
}
