use datavalue::OwnedDataValue;
use serde::ser::{Serialize, SerializeMap, Serializer};
use std::fmt;
use std::sync::Arc;

/// Canonical "Invalid Arguments" error message — used wherever an
/// operator rejects a malformed args list before evaluating.
pub(crate) const INVALID_ARGS: &str = "Invalid Arguments";

/// Canonical "NaN" string used as the `type` field of the thrown error
/// object that arithmetic and comparison ops raise on non-numeric input.
pub(crate) const NAN_ERROR: &str = "NaN";

/// Trait-object alias for the source carried by [`ErrorKind::Custom`].
/// Reference-counted so [`ErrorKind`] stays cheap to clone, and bounded
/// so a single `Error` value can be sent across threads.
pub type CustomSource = Arc<dyn std::error::Error + Send + Sync + 'static>;

/// String-only custom error — used by [`Error::custom_error`] to wrap a
/// bare message in a `dyn Error` shell.
#[derive(Debug)]
struct MessageError(String);

impl fmt::Display for MessageError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl std::error::Error for MessageError {}

/// Discriminant for [`Error`]. Stable variant tags are exposed via
/// [`Error::kind_tag`] for matching across releases.
#[non_exhaustive]
#[derive(Debug, Clone)]
pub enum ErrorKind {
    /// Invalid operator name
    InvalidOperator(String),
    /// Invalid arguments for an operator
    InvalidArguments(String),
    /// Variable not found in context
    VariableNotFound(String),
    /// Invalid context level access
    InvalidContextLevel(isize),
    /// Type conversion/coercion error
    TypeError(String),
    /// Arithmetic error (division by zero, overflow, etc.)
    ArithmeticError(String),
    /// Custom error for extensions. Carries the underlying typed error so
    /// callers can walk the source chain via [`std::error::Error::source`].
    /// Constructed via [`Error::custom_error`] (string-only) or
    /// [`Error::wrap`] (any `std::error::Error + Send + Sync + 'static`).
    Custom(CustomSource),
    /// JSON parsing/serialization error
    ParseError(String),
    /// Thrown error from throw operator
    Thrown(OwnedDataValue),
    /// Invalid format string or pattern
    FormatError(String),
    /// Index out of bounds for array operations
    IndexOutOfBounds { index: isize, length: usize },
    /// Invalid operator configuration
    ConfigurationError(String),
}

/// Error returned by every [`crate::Engine`] operation.
///
/// The `kind` field carries the failure category and any variant-specific
/// payload. `operator` and `path` are populated by the public `evaluate*`
/// entry points: `operator` names the outermost operator that produced the
/// error, and `path` is a breadcrumb of [`crate::CompiledNode`] ids from the
/// failure site toward the root (leaf-to-root). Use
/// [`Error::resolved_path`] to translate the ids into something the caller
/// can act on.
///
/// # Wire format
///
/// `Error` serialises as:
/// `{"type": <kind tag>, "message": <Display>, ...kind-extras, "operator"?, "path"?}`.
/// `operator` is omitted when `None`; `path` is omitted when empty. JS
/// consumers can `JSON.parse(err)` and switch on `err.type`.
#[non_exhaustive]
#[derive(Debug, Clone)]
pub struct Error {
    /// What went wrong.
    pub kind: ErrorKind,
    /// Outermost operator that produced this error, when known.
    pub operator: Option<String>,
    /// Breadcrumb of [`crate::CompiledNode`] ids from the failure site toward
    /// the root (leaf-to-root). Empty when the error came from parse/compile.
    pub path: Vec<u32>,
}

impl Error {
    /// Construct an [`Error`] with the given kind and no contextual metadata.
    #[inline]
    pub fn new(kind: ErrorKind) -> Self {
        Self {
            kind,
            operator: None,
            path: Vec::new(),
        }
    }

    /// Get a stable string tag for the error kind. Stable across releases.
    pub fn kind_tag(&self) -> &'static str {
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
    pub fn with_operator(mut self, operator: impl Into<String>) -> Self {
        self.operator = Some(operator.into());
        self
    }

    /// Attach the breadcrumb path and return self.
    pub fn with_path(mut self, path: Vec<u32>) -> Self {
        self.path = path;
        self
    }

    /// Resolve the raw [`Self::path`] node ids into structured
    /// [`crate::PathStep`]s (root-to-leaf). Walks the compiled tree once.
    ///
    /// Returns an empty vector when `self.path` is empty. Ids absent from the
    /// compiled tree (e.g. when the error came from compile-time, before
    /// evaluation populated the breadcrumb) are skipped.
    pub fn resolved_path(&self, compiled: &crate::Logic) -> Vec<crate::PathStep> {
        compiled.resolve_path(&self.path)
    }

    // ---- 4.x convenience constructors ----
    //
    // The pre-merge enum used `Error::Variant(x)` directly. With the merged
    // struct/enum split the right form is `ErrorKind::Variant(x).into()`.
    // The shorthand below keeps the 33 internal call sites readable without
    // pulling `ErrorKind` into every file's import list.

    /// Shorthand for `ErrorKind::InvalidOperator(name).into()`.
    #[inline]
    pub fn invalid_operator(name: impl Into<String>) -> Self {
        ErrorKind::InvalidOperator(name.into()).into()
    }
    /// Shorthand for `ErrorKind::InvalidArguments(msg).into()`.
    #[inline]
    pub fn invalid_arguments(msg: impl Into<String>) -> Self {
        ErrorKind::InvalidArguments(msg.into()).into()
    }
    /// Shorthand for `ErrorKind::VariableNotFound(name).into()`.
    #[inline]
    pub fn variable_not_found(name: impl Into<String>) -> Self {
        ErrorKind::VariableNotFound(name.into()).into()
    }
    /// Shorthand for `ErrorKind::InvalidContextLevel(level).into()`.
    #[inline]
    pub fn invalid_context_level(level: isize) -> Self {
        ErrorKind::InvalidContextLevel(level).into()
    }
    /// Shorthand for `ErrorKind::TypeError(msg).into()`.
    #[inline]
    pub fn type_error(msg: impl Into<String>) -> Self {
        ErrorKind::TypeError(msg.into()).into()
    }
    /// Shorthand for `ErrorKind::ArithmeticError(msg).into()`.
    #[inline]
    pub fn arithmetic_error(msg: impl Into<String>) -> Self {
        ErrorKind::ArithmeticError(msg.into()).into()
    }
    /// Shorthand for a message-only [`ErrorKind::Custom`]. Equivalent to
    /// [`Self::wrap`] with a string-shaped error inside.
    #[inline]
    pub fn custom(msg: impl Into<String>) -> Self {
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
    pub fn parse_error(msg: impl Into<String>) -> Self {
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
    pub fn format_error(msg: impl Into<String>) -> Self {
        ErrorKind::FormatError(msg.into()).into()
    }
    /// Shorthand for `ErrorKind::IndexOutOfBounds { index, length }.into()`.
    #[inline]
    pub fn index_out_of_bounds(index: isize, length: usize) -> Self {
        ErrorKind::IndexOutOfBounds { index, length }.into()
    }
    /// Shorthand for `ErrorKind::ConfigurationError(msg).into()`.
    #[inline]
    pub fn configuration_error(msg: impl Into<String>) -> Self {
        ErrorKind::ConfigurationError(msg.into()).into()
    }

    /// Canonical "Invalid Arguments" error. Used wherever an operator
    /// rejects malformed args before evaluating.
    #[inline]
    pub(crate) fn invalid_args() -> Self {
        Error::invalid_arguments(INVALID_ARGS)
    }

    /// Canonical NaN error — `{"type": "NaN"}` thrown via [`Error::thrown`].
    /// Used by arithmetic and comparison ops on non-numeric input.
    #[inline]
    pub(crate) fn nan() -> Self {
        Error::thrown(OwnedDataValue::object([("type", NAN_ERROR)]))
    }
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        // Render the kind first, then optionally the operator context.
        write_kind_message(f, &self.kind)?;
        if let Some(op) = &self.operator {
            write!(f, " (in operator: {})", op)?;
        }
        Ok(())
    }
}

/// Render the `ErrorKind` portion of an error message, without the operator
/// suffix. Single source of truth for the kind → human-readable mapping; used
/// by `Display for Error` (which then appends the operator context) and
/// `Error::serialize` (via `KindDisplay`).
fn write_kind_message(f: &mut fmt::Formatter<'_>, kind: &ErrorKind) -> fmt::Result {
    match kind {
        ErrorKind::InvalidOperator(op) => write!(f, "Invalid operator: {}", op),
        ErrorKind::InvalidArguments(msg) => write!(f, "Invalid arguments: {}", msg),
        ErrorKind::VariableNotFound(var) => write!(f, "Variable not found: {}", var),
        ErrorKind::InvalidContextLevel(level) => write!(f, "Invalid context level: {}", level),
        ErrorKind::TypeError(msg) => write!(f, "Type error: {}", msg),
        ErrorKind::ArithmeticError(msg) => write!(f, "Arithmetic error: {}", msg),
        ErrorKind::Custom(err) => write!(f, "{}", err),
        ErrorKind::ParseError(msg) => write!(f, "Parse error: {}", msg),
        ErrorKind::Thrown(val) => {
            #[cfg(feature = "compat")]
            {
                let json = crate::compat::owned_to_serde(val);
                write!(f, "Thrown: {}", json)
            }
            #[cfg(not(feature = "compat"))]
            {
                write!(f, "Thrown: {:?}", val)
            }
        }
        ErrorKind::FormatError(msg) => write!(f, "Format error: {}", msg),
        ErrorKind::IndexOutOfBounds { index, length } => write!(
            f,
            "Index {} out of bounds for array of length {}",
            index, length
        ),
        ErrorKind::ConfigurationError(msg) => write!(f, "Configuration error: {}", msg),
    }
}

impl std::error::Error for Error {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match &self.kind {
            ErrorKind::Custom(err) => Some(err.as_ref()),
            _ => None,
        }
    }
}

#[cfg(feature = "compat")]
impl From<serde_json::Error> for Error {
    fn from(err: serde_json::Error) -> Self {
        Error::new(ErrorKind::ParseError(err.to_string()))
    }
}

impl From<datavalue::ParseError> for Error {
    fn from(err: datavalue::ParseError) -> Self {
        Error::new(ErrorKind::ParseError(err.to_string()))
    }
}

impl From<ErrorKind> for Error {
    #[inline]
    fn from(kind: ErrorKind) -> Self {
        Error::new(kind)
    }
}

impl Serialize for Error {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        // Shape:
        // { "type": <tag>, "message": <Display>, ...kind-extras, "operator"?, "path"? }
        let mut map = serializer.serialize_map(None)?;
        map.serialize_entry("type", self.kind_tag())?;
        // The Display impl appends "(in operator: ...)" when set; for the
        // `message` field we want the kind portion only, so render kind
        // without the operator suffix.
        map.serialize_entry("message", &KindDisplay(&self.kind).to_string())?;
        match &self.kind {
            ErrorKind::VariableNotFound(name) => map.serialize_entry("variable", name)?,
            ErrorKind::InvalidContextLevel(level) => map.serialize_entry("level", level)?,
            ErrorKind::Thrown(value) => map.serialize_entry("thrown", value)?,
            ErrorKind::IndexOutOfBounds { index, length } => {
                map.serialize_entry("index", index)?;
                map.serialize_entry("length", length)?;
            }
            _ => {}
        }
        if let Some(op) = &self.operator {
            map.serialize_entry("operator", op)?;
        }
        if !self.path.is_empty() {
            map.serialize_entry("path", &self.path)?;
        }
        map.end()
    }
}

/// Render an [`ErrorKind`] without the operator suffix. Used by
/// [`Error::serialize`] to populate the `message` field.
struct KindDisplay<'a>(&'a ErrorKind);

impl<'a> fmt::Display for KindDisplay<'a> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write_kind_message(f, self.0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn wrap_renders_via_display() {
        let io_err = std::io::Error::new(std::io::ErrorKind::NotFound, "missing key");
        let err = Error::wrap(io_err);
        assert_eq!(err.kind_tag(), "Custom");
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
        assert_eq!(wrapped.kind_tag(), "VariableNotFound");
        assert!(matches!(wrapped.kind, ErrorKind::VariableNotFound(ref name) if name == "x"));
        // operator/path metadata round-trips too.
        let with_meta = inner.with_operator("var").with_path(vec![1, 2, 3]);
        let wrapped = Error::wrap(with_meta);
        assert_eq!(wrapped.operator.as_deref(), Some("var"));
        assert_eq!(wrapped.path, vec![1, 2, 3]);
    }
}
