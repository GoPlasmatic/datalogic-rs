//! `Error` — the unified error type returned by every public engine
//! operation. Submodules are structured by concern:
//!
//! - [`kind`] — `ErrorKind` enum + `CustomErrorSource` trait alias.
//! - [`path`] — `ErrorPath`, the internal breadcrumb storage.
//! - [`serde`] — `Display`, `std::error::Error`, `Serialize`, and `From`
//!   impls for foreign error types.
//!
//! Re-exports below preserve the pre-split `crate::error::*` import paths so
//! callers elsewhere in the crate are unaffected by the file split.

mod kind;
mod path;
mod serde;

pub use kind::{CustomErrorSource, ErrorKind};
pub(crate) use path::ErrorPath;

use datavalue::OwnedDataValue;
use std::borrow::Cow;
use std::fmt;
use std::sync::Arc;

/// Canonical "Invalid Arguments" error message — used wherever an
/// operator rejects a malformed args list before evaluating.
pub(crate) const INVALID_ARGS: &str = "Invalid Arguments";

/// Canonical "NaN" string used as the `type` field of the thrown error
/// object that arithmetic and comparison ops raise on non-numeric input.
pub(crate) const NAN_ERROR: &str = "NaN";

/// String-only custom error — used by [`Error::custom_message`] to wrap a
/// bare message in a `dyn Error` shell.
#[derive(Debug)]
struct MessageError(String);

impl fmt::Display for MessageError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl std::error::Error for MessageError {}

/// Error returned by every [`crate::Engine`] operation.
///
/// The `kind` field carries the failure category and any variant-specific
/// payload. `operator` and `node_ids` are populated by the public
/// `evaluate*` entry points: `operator` names the outermost operator that
/// produced the error, and `node_ids` is a breadcrumb of compiled-node ids
/// from the failure site toward the root (leaf-to-root). Use
/// [`Error::resolve_path`] to translate the ids into structured
/// [`crate::PathStep`]s callers can act on.
///
/// # Wire format
///
/// `Error` serialises as:
/// `{"type": <kind tag>, "message": <Display>, ...kind-extras, "operator"?, "node_ids"?}`.
/// `operator` is omitted when `None`; `node_ids` is omitted when empty. JS
/// consumers can `JSON.parse(err)` and switch on `err.type`.
///
/// # Source chains
///
/// `std::error::Error::source` returns `Some` only for [`ErrorKind::Custom`]
/// — the variant produced by [`Error::wrap`]. Every other variant carries
/// a flat string or structured payload, not a typed cause. To attach a
/// typed source error, wrap it via `Error::wrap` instead of constructing
/// e.g. `Error::invalid_arguments("...")` directly.
#[non_exhaustive]
#[derive(Debug, Clone)]
pub struct Error {
    /// What went wrong. Pattern-matched by callers; stays public.
    pub kind: ErrorKind,
    /// Outermost operator that produced this error, when known.
    /// Read via [`Self::operator`]. Stored as `Cow<'static, str>` so
    /// built-in op names (the dominant case) are zero-allocation
    /// `Cow::Borrowed` references; only dynamic custom-operator names
    /// carry an owned `String` via `Cow::Owned`.
    operator: Option<Cow<'static, str>>,
    /// Breadcrumb of compiled-node ids from the failure site toward the
    /// root (leaf-to-root). Empty when the error came from parse/compile
    /// or wasn't routed through the public `evaluate*` path. Stored
    /// inline (no `Box`) so attaching the breadcrumb at the boundary is
    /// just a move — heap-allocating per error showed up as a +30%
    /// regression on error-heavy suites (try/throw/datetime/string).
    node_ids: ErrorPath,
}

impl Error {
    /// Construct an [`Error`] with the given kind and no contextual metadata.
    #[inline]
    pub fn new(kind: ErrorKind) -> Self {
        Self {
            kind,
            operator: None,
            node_ids: ErrorPath::new(),
        }
    }

    /// Outermost operator that produced this error, when known.
    /// Returns `None` for parse/compile errors and for raw constructor sites
    /// that didn't call [`Self::with_operator`].
    #[inline]
    pub fn operator(&self) -> Option<&str> {
        self.operator.as_deref()
    }

    /// Breadcrumb of compiled-node ids from the failure site toward the root
    /// (leaf-to-root). Returns an empty slice when the error came from
    /// parse/compile or wasn't routed through the public `evaluate*` path.
    /// Use [`Self::resolve_path`] to convert ids into named [`crate::PathStep`]s.
    #[inline]
    pub fn node_ids(&self) -> &[u32] {
        self.node_ids.as_slice()
    }

    /// Get a stable string tag for the error kind. Stable across releases.
    pub fn tag(&self) -> &'static str {
        match self.kind {
            ErrorKind::InvalidOperator(_) => "InvalidOperator",
            ErrorKind::InvalidArguments(_) => "InvalidArguments",
            ErrorKind::VariableNotFound(_) => "VariableNotFound",
            ErrorKind::InvalidContextLevel(_) => "InvalidContextLevel",
            ErrorKind::TypeError(_) => "TypeError",
            ErrorKind::ArithmeticError(_) => "ArithmeticError",
            ErrorKind::Custom(_) => "Custom",
            ErrorKind::ParseError(_) => "ParseError",
            ErrorKind::Thrown(_) => "Thrown",
            ErrorKind::FormatError(_) => "FormatError",
            ErrorKind::IndexOutOfBounds { .. } => "IndexOutOfBounds",
            ErrorKind::ConfigurationError(_) => "ConfigurationError",
        }
    }

    /// Attach the outermost operator name and return self.
    ///
    /// Accepts anything convertible to `Cow<'static, str>` — passing a
    /// `&'static str` literal stays zero-allocation; a `String` becomes
    /// `Cow::Owned` (one move, no copy).
    #[must_use = "builder methods return the modified Error; bind or return it"]
    pub fn with_operator(mut self, operator: impl Into<Cow<'static, str>>) -> Self {
        self.operator = Some(operator.into());
        self
    }

    /// Attach the breadcrumb path and return self.
    ///
    /// Takes a `Vec<u32>` of compiled-node ids (leaf-to-root). The internal
    /// storage is currently a plain `Vec<u32>`; future versions may swap to
    /// an inline-buffer / smallvec layout without an API change.
    #[must_use = "builder methods return the modified Error; bind or return it"]
    pub fn with_node_ids(mut self, ids: Vec<u32>) -> Self {
        self.node_ids = ids.into();
        self
    }

    /// Resolve the raw [`Self::node_ids`] breadcrumb into structured
    /// [`crate::PathStep`]s (root-to-leaf). Walks the compiled tree once.
    ///
    /// Returns an empty vector when `self.node_ids` is empty. Ids absent
    /// from the compiled tree (e.g. when the error came from compile-time,
    /// before evaluation populated the breadcrumb) are skipped.
    ///
    /// **Why on demand**: an earlier design eagerly cached the resolved
    /// steps on `Error` so callers could read them without holding the
    /// `Logic`. That walk allocates a HashMap of every node + a `String`
    /// JSON pointer per node, and paying it on every boundary error
    /// inflated error-heavy benchmark suites by 17×. Resolving on demand
    /// at the catch site puts the cost where the caller actually needs
    /// the data — and most callers either inspect raw [`Self::node_ids`]
    /// only, or already hold the compiled `Logic` at the catch site.
    pub fn resolve_path(&self, compiled: &crate::Logic) -> Vec<crate::PathStep> {
        compiled.resolve_node_ids(self.node_ids.as_slice())
    }

    // ---- 4.x convenience constructors ----
    //
    // The pre-merge enum used `Error::Variant(x)` directly. With the merged
    // struct/enum split the right form is `ErrorKind::Variant(x).into()`.
    // The shorthand below keeps the 33 internal call sites readable without
    // pulling `ErrorKind` into every file's import list.

    /// Shorthand for `ErrorKind::InvalidOperator(name).into()`.
    #[inline]
    pub fn invalid_operator(name: impl Into<Cow<'static, str>>) -> Self {
        ErrorKind::InvalidOperator(name.into()).into()
    }
    /// Shorthand for `ErrorKind::InvalidArguments(msg).into()`.
    #[inline]
    pub fn invalid_arguments(msg: impl Into<Cow<'static, str>>) -> Self {
        ErrorKind::InvalidArguments(msg.into()).into()
    }
    /// Shorthand for `ErrorKind::VariableNotFound(name).into()`.
    #[inline]
    pub fn variable_not_found(name: impl Into<Cow<'static, str>>) -> Self {
        ErrorKind::VariableNotFound(name.into()).into()
    }
    /// Shorthand for `ErrorKind::InvalidContextLevel(level).into()`.
    #[inline]
    pub fn invalid_context_level(level: isize) -> Self {
        ErrorKind::InvalidContextLevel(level).into()
    }
    /// Shorthand for `ErrorKind::TypeError(msg).into()`.
    #[inline]
    pub fn type_error(msg: impl Into<Cow<'static, str>>) -> Self {
        ErrorKind::TypeError(msg.into()).into()
    }
    /// Shorthand for `ErrorKind::ArithmeticError(msg).into()`.
    #[inline]
    pub fn arithmetic_error(msg: impl Into<Cow<'static, str>>) -> Self {
        ErrorKind::ArithmeticError(msg.into()).into()
    }
    /// Shorthand for a message-only [`ErrorKind::Custom`]. Equivalent to
    /// [`Self::wrap`] with a string-shaped error inside. Reach for
    /// [`Self::wrap`] directly when you have a typed `std::error::Error`
    /// to preserve.
    #[inline]
    pub fn custom_message(msg: impl Into<String>) -> Self {
        Self::wrap(MessageError(msg.into()))
    }

    /// Wrap any `std::error::Error + Send + Sync + 'static` into an
    /// [`ErrorKind::Custom`], preserving the source chain so consumers can
    /// walk it via [`std::error::Error::source`]:
    ///
    /// ```ignore
    /// some_io_call().map_err(Error::wrap)?;
    /// ```
    ///
    /// The original error stays inspectable: `error.source()` returns
    /// `Some(&original)`. Standard chain-walking via
    /// [`std::error::Error::source`] applies all the way down.
    ///
    /// Wrapping an existing [`Error`] is a no-op — the input is returned
    /// unchanged rather than producing `Custom(Custom(...))`.
    #[inline]
    pub fn wrap<E: std::error::Error + Send + Sync + 'static>(err: E) -> Self {
        // No-op when E is already `Error`. We hold `err` inside an `Option`
        // and downcast that — `TypeId::of::<Option<E>>() == TypeId::of::<Option<Error>>()`
        // iff `E == Error`, so the downcast succeeds exactly when we'd
        // otherwise double-wrap.
        let mut slot: Option<E> = Some(err);
        if let Some(slot_as_error) =
            (&mut slot as &mut dyn std::any::Any).downcast_mut::<Option<Error>>()
        {
            return slot_as_error.take().expect("just stored `Some`");
        }
        let err = slot.take().expect("just stored `Some`");
        ErrorKind::Custom(Arc::new(err)).into()
    }
    /// Shorthand for `ErrorKind::ParseError(msg).into()`.
    #[inline]
    pub fn parse_error(msg: impl Into<Cow<'static, str>>) -> Self {
        ErrorKind::ParseError(msg.into()).into()
    }
    /// Shorthand for `ErrorKind::Thrown(value).into()`.
    #[inline]
    pub fn thrown(value: OwnedDataValue) -> Self {
        ErrorKind::Thrown(value).into()
    }

    /// If this is an [`ErrorKind::Thrown`], return its payload. Convenience
    /// accessor so consumers (loggers, structured-error walkers, the test
    /// runner) don't have to pattern-match on the kind themselves.
    #[inline]
    pub fn thrown_value(&self) -> Option<&OwnedDataValue> {
        if let ErrorKind::Thrown(v) = &self.kind {
            Some(v)
        } else {
            None
        }
    }
    /// Shorthand for `ErrorKind::FormatError(msg).into()`.
    #[inline]
    pub fn format_error(msg: impl Into<Cow<'static, str>>) -> Self {
        ErrorKind::FormatError(msg.into()).into()
    }
    /// Shorthand for `ErrorKind::IndexOutOfBounds { index, length }.into()`.
    #[inline]
    pub fn index_out_of_bounds(index: isize, length: usize) -> Self {
        ErrorKind::IndexOutOfBounds { index, length }.into()
    }
    /// Shorthand for `ErrorKind::ConfigurationError(msg).into()`.
    #[inline]
    pub fn configuration_error(msg: impl Into<Cow<'static, str>>) -> Self {
        ErrorKind::ConfigurationError(msg.into()).into()
    }

    /// Canonical "Invalid Arguments" error. Used wherever an operator
    /// rejects malformed args before evaluating.
    #[inline]
    pub(crate) fn invalid_args() -> Self {
        Error::invalid_arguments(INVALID_ARGS)
    }

    /// Decorate an error from a public `evaluate*` boundary with the
    /// breadcrumb path (raw ids only — see below) and the outermost
    /// operator name. Marked `#[cold]` + `#[inline(never)]` so the
    /// dispatch caller's `Err` arm shrinks to a single call instruction,
    /// keeping the hot `Ok` arm's I-cache footprint tight.
    ///
    /// **Lazy path resolution.** The boundary attaches raw compiled-node
    /// ids only — it does *not* call `Logic::resolve_node_ids` here. That
    /// walk allocates a HashMap of every node + a `String` JSON pointer
    /// per node and was measured to balloon try.json from 51 ns/op to
    /// 898 ns/op (17×) and arithmetic/plus.json from 22 to 84 ns
    /// (4×) on error-heavy suites where every iteration constructs an
    /// Error. Consumers that need structured steps call
    /// [`Self::resolve_path`] (takes a `&Logic`) on demand, which is
    /// the same cost paid once at the catch site rather than at every
    /// boundary crossing.
    ///
    /// `prefer_existing_op` controls whether to fall back to
    /// `compiled.root_op_name` when no operator was already attached:
    /// the `Engine::evaluate*` sites pass `true` (only attach if a
    /// deeper site didn't name a more specific failing op);
    /// `TracedSession` passes `false` to preserve its prior
    /// unconditional-overwrite behavior.
    #[cold]
    #[inline(never)]
    pub(crate) fn decorated(
        mut self,
        node_ids: Vec<u32>,
        compiled: &crate::Logic,
        prefer_existing_op: bool,
    ) -> Self {
        self = self.with_node_ids(node_ids);
        if !prefer_existing_op || self.operator.is_none() {
            if let Some(name) = compiled.root_op_name.clone() {
                self.operator = Some(name);
            }
        }
        self
    }

    /// Canonical NaN error — `{"type": "NaN"}` thrown via [`Error::thrown`].
    /// Used by arithmetic and comparison ops on non-numeric input.
    #[inline]
    pub(crate) fn nan() -> Self {
        Error::thrown(OwnedDataValue::object([("type", NAN_ERROR)]))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn wrap_renders_via_display() {
        let io_err = std::io::Error::new(std::io::ErrorKind::NotFound, "missing key");
        let err = Error::wrap(io_err);
        assert_eq!(err.tag(), "Custom");
        assert!(err.to_string().contains("missing key"));
    }

    #[test]
    fn wrap_preserves_source_chain() {
        use std::error::Error as _;
        let io_err = std::io::Error::new(std::io::ErrorKind::NotFound, "missing key");
        let err = Error::wrap(io_err);
        // `Error::source` returns the original typed error so consumers can
        // walk the chain — the previous Display-only `wrap` lost this.
        let src = err.source().expect("Custom should expose its source");
        assert!(src.to_string().contains("missing key"));
        // And the source itself can be downcast to the original type.
        assert!(src.downcast_ref::<std::io::Error>().is_some());
    }

    #[test]
    fn wrap_threads_through_question_mark() {
        // Smoke test for the `?` ergonomic — `Error::wrap` slots into a
        // `map_err` chain so foreign errors flow up unchanged.
        fn inner() -> std::result::Result<(), Error> {
            "not_an_int".parse::<i32>().map_err(Error::wrap)?;
            Ok(())
        }
        let err = inner().expect_err("parse should fail");
        assert!(matches!(err.kind, ErrorKind::Custom(_)));
    }

    #[test]
    fn wrap_of_existing_error_is_noop() {
        // `Error::wrap(some_error)` would otherwise produce `Custom(Custom(...))`
        // — the no-op short-circuit returns the input unchanged.
        let inner = Error::variable_not_found("x");
        let wrapped = Error::wrap(inner.clone());
        assert_eq!(wrapped.tag(), "VariableNotFound");
        assert!(matches!(wrapped.kind, ErrorKind::VariableNotFound(ref name) if name == "x"));
        // operator + node_ids metadata round-trip too.
        let with_meta = inner.with_operator("var").with_node_ids(vec![1, 2, 3]);
        let wrapped = Error::wrap(with_meta);
        assert_eq!(wrapped.operator(), Some("var"));
        assert_eq!(wrapped.node_ids(), &[1, 2, 3]);
    }

    #[test]
    fn error_path_default_is_empty() {
        let p = ErrorPath::new();
        assert!(p.as_slice().is_empty());
        assert_eq!(p.as_slice(), &[] as &[u32]);
    }

    #[test]
    fn error_path_from_vec_round_trips() {
        let p: ErrorPath = vec![10, 20, 30].into();
        assert_eq!(p.as_slice(), &[10, 20, 30]);
    }

    #[test]
    fn with_node_ids_stores_inline_no_box() {
        // Engine boundary calls `with_node_ids` once per error; storage
        // is inline `Vec<u32>` so this is just a move, not a heap alloc.
        let err = Error::invalid_arguments("x").with_node_ids(vec![1, 2, 3]);
        assert_eq!(err.node_ids(), &[1, 2, 3]);
    }
}
