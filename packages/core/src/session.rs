//! Reusable evaluation handle that hides arena lifecycle from callers.
//!
//! [`Session`] owns a [`bumpalo::Bump`] and resets it at the start of every
//! call, so peak memory is bounded by the largest single evaluation while
//! amortising allocator cost across many calls. Inputs go through
//! [`crate::EvalInput`] so callers pass whatever they have on hand
//! (`&str`, `&OwnedDataValue`, `&serde_json::Value`, …); outputs are owned
//! ([`OwnedDataValue`] / `String` / `serde_json::Value`) so they outlive the
//! next reset.
//!
//! For callers who want zero-copy borrows into the arena (and are willing to
//! manage the [`bumpalo::Bump`] themselves), use [`crate::Engine::evaluate`]
//! directly — that path returns `&DataValue<'a>` and avoids the deep-clone on
//! the way out.

use bumpalo::Bump;
use datavalue::OwnedDataValue;

use crate::{Engine, EvalInput, Logic, Result};

/// Reusable evaluation handle. Construct via [`Engine::session`].
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

    /// Evaluate `compiled` against `data` and deep-clone the result into an
    /// [`OwnedDataValue`] that survives the next reset.
    ///
    /// Resets the internal arena before each call, so peak memory tracks the
    /// largest single evaluation rather than the lifetime sum.
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
        self.arena.reset();
        let arena: &'a Bump = &self.arena;
        let av = data.into_arena_value(arena)?;
        let result = self.engine.evaluate(compiled, av, arena)?;
        Ok(result.to_owned())
    }

    /// JSON-string convenience: evaluate `compiled` against a JSON-encoded
    /// `data` payload and serialise the result back to a JSON `String`.
    /// Mirrors [`Engine::evaluate_str`] but reuses the arena across calls.
    pub fn evaluate_str(&mut self, compiled: &Logic, data: &str) -> Result<String> {
        self.arena.reset();
        let arena: &Bump = &self.arena;
        let av = data.into_arena_value(arena)?;
        let result = self.engine.evaluate(compiled, av, arena)?;
        Ok(crate::arena::data_to_json_string(result))
    }

    /// `serde_json::Value` convenience: evaluate `compiled` against a serde
    /// value and convert the result back to a serde value. Mirrors
    /// [`Engine::evaluate_serde`] but reuses the arena across calls.
    #[cfg(feature = "compat")]
    pub fn evaluate_serde(
        &mut self,
        compiled: &Logic,
        data: &serde_json::Value,
    ) -> Result<serde_json::Value> {
        self.arena.reset();
        let arena: &Bump = &self.arena;
        let av = data.into_arena_value(arena)?;
        let result = self.engine.evaluate(compiled, av, arena)?;
        Ok(crate::arena::data_to_value(result))
    }
}
