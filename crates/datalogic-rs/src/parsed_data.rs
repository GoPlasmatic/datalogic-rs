//! [`ParsedData`] — a self-contained parsed JSON document.
//!
//! Parsing the data JSON dominates the string-in/string-out contract
//! (70-90% of a parse-eval-serialize round trip in the boundary
//! measurements), and every string-shaped entry point re-parses the
//! same payload on every call. `ParsedData` factors that cost out:
//! parse once, then evaluate any number of rules against the resident
//! tree through the zero-cost `&DataValue` passthrough of
//! [`crate::EvalInput`].
//!
//! Internally this is the same self-referential shape as the compiler's
//! pre-built literals (`node/prelit.rs`): a [`self_cell`] owning a
//! [`Bump`] arena and the [`DataValue`] tree parsed into it. The input
//! text is copied into the arena before parsing, so the tree borrows
//! only from memory the cell owns and the handle is fully
//! self-contained.
//!
//! `ParsedData` is `Send` (move it across threads freely) but not
//! `Sync` — [`Bump`] is `!Sync`. A binding that wants to share one
//! handle across threads must layer its own guarantee on top (the tree
//! is never mutated after construction, so read-only sharing is sound
//! when the wrapper enforces it).

use bumpalo::Bump;
use self_cell::self_cell;

use crate::Result;
use crate::arena::DataValue;

self_cell!(
    /// Owns the bump arena and the `DataValue` tree parsed into it.
    struct ParsedCell {
        owner: Bump,
        #[covariant]
        dependent: DataValue,
    }
);

/// A parsed JSON document that owns its backing storage.
///
/// Accepted by every arena-lifetime evaluation entry point
/// ([`crate::Engine::evaluate`], [`crate::Session::eval`] /
/// [`crate::Session::eval_borrowed`], …) via [`crate::EvalInput`] at
/// zero per-call conversion cost — the tree is already arena-resident.
///
/// # Example
///
/// ```rust
/// use datalogic_rs::{Engine, ParsedData};
/// use datalogic_rs::bumpalo::Bump;
///
/// let engine = Engine::new();
/// let data = ParsedData::from_json(r#"{"user": {"age": 34}}"#).unwrap();
///
/// // Evaluate many rules against the one parsed payload.
/// let adult = engine.compile(r#"{">=": [{"var": "user.age"}, 18]}"#).unwrap();
/// let senior = engine.compile(r#"{">=": [{"var": "user.age"}, 65]}"#).unwrap();
///
/// let arena = Bump::new();
/// assert_eq!(engine.evaluate(&adult, &data, &arena).unwrap().as_bool(), Some(true));
/// assert_eq!(engine.evaluate(&senior, &data, &arena).unwrap().as_bool(), Some(false));
/// ```
pub struct ParsedData(ParsedCell);

impl ParsedData {
    /// Parse `json` into a self-contained document.
    ///
    /// Returns the engine's usual `ParseError` on malformed input.
    pub fn from_json(json: &str) -> Result<Self> {
        // Copy the input into the arena first: the parser's zero-copy
        // strings then borrow from arena-owned bytes, which is what
        // keeps the cell self-contained. Capacity heuristic: input copy
        // plus tree nodes typically land within ~2x the text size.
        let cell =
            ParsedCell::try_new(Bump::with_capacity(json.len().saturating_mul(2)), |arena| {
                let stable: &str = arena.alloc_str(json);
                DataValue::from_str(stable, arena).map_err(crate::Error::from)
            })?;
        Ok(Self(cell))
    }

    /// Borrow the parsed tree.
    ///
    /// The returned reference is valid for as long as the handle lives;
    /// it satisfies the `&DataValue` input shape of the evaluation
    /// entry points directly.
    pub fn value(&self) -> &DataValue<'_> {
        self.0.borrow_dependent()
    }

    /// Bytes currently held by the backing arena (input copy + tree).
    pub fn allocated_bytes(&self) -> usize {
        self.0.borrow_owner().allocated_bytes()
    }
}

impl std::fmt::Debug for ParsedData {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_tuple("ParsedData").field(self.value()).finish()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Engine;

    #[test]
    fn parses_and_exposes_value() {
        let data = ParsedData::from_json(r#"{"a": [1, 2, 3], "b": "text"}"#).unwrap();
        let v = data.value();
        assert!(v.is_object());
        assert_eq!(v.to_string(), r#"{"a":[1,2,3],"b":"text"}"#);
        assert!(data.allocated_bytes() > 0);
    }

    #[test]
    fn malformed_json_is_a_parse_error() {
        let err = ParsedData::from_json("{ not json").unwrap_err();
        assert_eq!(err.tag(), "ParseError");
    }

    #[test]
    fn evaluates_through_engine_and_session() {
        let engine = Engine::new();
        let rule = engine.compile(r#"{"+": [{"var": "x"}, 1]}"#).unwrap();
        let data = ParsedData::from_json(r#"{"x": 41}"#).unwrap();

        // Engine::evaluate (caller arena) — repeated calls, one parse.
        let arena = bumpalo::Bump::new();
        for _ in 0..3 {
            let out = engine.evaluate(&rule, &data, &arena).unwrap();
            assert_eq!(out.as_i64(), Some(42));
        }

        // Session::eval_borrowed — same handle, session-owned arena.
        let mut session = engine.session();
        let out = session.eval_borrowed(&rule, &data).unwrap();
        assert_eq!(out.as_i64(), Some(42));
    }

    #[test]
    fn outlives_short_lived_eval_arenas() {
        let engine = Engine::new();
        let rule = engine.compile(r#"{"var": "name"}"#).unwrap();
        let data = ParsedData::from_json(r#"{"name": "Ada"}"#).unwrap();
        for _ in 0..2 {
            let arena = bumpalo::Bump::new();
            let out = engine.evaluate(&rule, &data, &arena).unwrap();
            assert_eq!(out.to_string(), r#""Ada""#);
            // arena drops here; `data` stays valid.
        }
        assert_eq!(data.value().to_string(), r#"{"name":"Ada"}"#);
    }

    #[test]
    fn handle_is_send() {
        fn assert_send<T: Send>() {}
        assert_send::<ParsedData>();
    }
}
