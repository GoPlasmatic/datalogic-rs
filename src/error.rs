use datavalue::OwnedDataValue;
use serde::ser::{Serialize, SerializeMap, Serializer};
use std::fmt;

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
    /// Custom error for extensions
    Custom(String),
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

/// Error returned by every [`crate::DataLogic`] operation.
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
    pub fn resolved_path(&self, compiled: &crate::CompiledLogic) -> Vec<crate::PathStep> {
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
    /// Shorthand for `ErrorKind::Custom(msg).into()`.
    #[inline]
    pub fn custom(msg: impl Into<String>) -> Self {
        ErrorKind::Custom(msg.into()).into()
    }

    /// Wrap any `impl Display` (typically a foreign error type) into an
    /// [`ErrorKind::Custom`]. Lets custom-operator authors propagate I/O / DB
    /// / HTTP failures via the standard `?` chain:
    ///
    /// ```ignore
    /// some_io_call().map_err(Error::wrap)?;
    /// ```
    ///
    /// Source error chains are not preserved (the wrapped value is captured
    /// as a `String` via `Display`). For error-chain inspection, build a
    /// custom `ErrorKind` variant in your application instead.
    #[inline]
    pub fn wrap<E: fmt::Display>(err: E) -> Self {
        ErrorKind::Custom(err.to_string()).into()
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
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        // Render the kind first, then optionally the operator context. Mirrors
        // the historical `StructuredError` Display so existing log output
        // shapes don't shift.
        match &self.kind {
            ErrorKind::InvalidOperator(op) => write!(f, "Invalid operator: {}", op)?,
            ErrorKind::InvalidArguments(msg) => write!(f, "Invalid arguments: {}", msg)?,
            ErrorKind::VariableNotFound(var) => write!(f, "Variable not found: {}", var)?,
            ErrorKind::InvalidContextLevel(level) => write!(f, "Invalid context level: {}", level)?,
            ErrorKind::TypeError(msg) => write!(f, "Type error: {}", msg)?,
            ErrorKind::ArithmeticError(msg) => write!(f, "Arithmetic error: {}", msg)?,
            ErrorKind::Custom(msg) => write!(f, "{}", msg)?,
            ErrorKind::ParseError(msg) => write!(f, "Parse error: {}", msg)?,
            ErrorKind::Thrown(val) => {
                #[cfg(feature = "compat")]
                {
                    let json = crate::value::owned_to_serde(val);
                    write!(f, "Thrown: {}", json)?;
                }
                #[cfg(not(feature = "compat"))]
                {
                    write!(f, "Thrown: {:?}", val)?;
                }
            }
            ErrorKind::FormatError(msg) => write!(f, "Format error: {}", msg)?,
            ErrorKind::IndexOutOfBounds { index, length } => write!(
                f,
                "Index {} out of bounds for array of length {}",
                index, length
            )?,
            ErrorKind::ConfigurationError(msg) => write!(f, "Configuration error: {}", msg)?,
        }
        if let Some(op) = &self.operator {
            write!(f, " (in operator: {})", op)?;
        }
        Ok(())
    }
}

impl std::error::Error for Error {}

#[cfg(feature = "compat")]
impl From<serde_json::Error> for Error {
    fn from(err: serde_json::Error) -> Self {
        Error::new(ErrorKind::ParseError(err.to_string()))
    }
}

impl From<datavalue::ParseError> for Error {
    fn from(err: datavalue::ParseError) -> Self {
        Error::new(ErrorKind::ParseError(format!("{:?}", err)))
    }
}

impl From<ErrorKind> for Error {
    #[inline]
    fn from(kind: ErrorKind) -> Self {
        Error::new(kind)
    }
}

impl Default for Error {
    fn default() -> Self {
        Error::new(ErrorKind::Custom(String::new()))
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
        match self.0 {
            ErrorKind::InvalidOperator(op) => write!(f, "Invalid operator: {}", op),
            ErrorKind::InvalidArguments(msg) => write!(f, "Invalid arguments: {}", msg),
            ErrorKind::VariableNotFound(var) => write!(f, "Variable not found: {}", var),
            ErrorKind::InvalidContextLevel(level) => write!(f, "Invalid context level: {}", level),
            ErrorKind::TypeError(msg) => write!(f, "Type error: {}", msg),
            ErrorKind::ArithmeticError(msg) => write!(f, "Arithmetic error: {}", msg),
            ErrorKind::Custom(msg) => write!(f, "{}", msg),
            ErrorKind::ParseError(msg) => write!(f, "Parse error: {}", msg),
            ErrorKind::Thrown(val) => {
                #[cfg(feature = "compat")]
                {
                    let json = crate::value::owned_to_serde(val);
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
}

/// Deprecated alias for [`Error`]. Pre-merge, structured-error-bearing entry
/// points returned `Result<_, StructuredError>`; today they return
/// `Result<_, Error>` and `Error` always carries the operator/path metadata.
/// Kept so 4.x callers and the `compat` shims keep compiling.
#[deprecated(
    since = "5.0.0",
    note = "use `Error`; `StructuredError` is now a type alias and will be removed in 5.1"
)]
pub type StructuredError = Error;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn wrap_stringifies_via_display() {
        let io_err = std::io::Error::new(std::io::ErrorKind::NotFound, "missing key");
        let err = Error::wrap(io_err);
        assert_eq!(err.kind_tag(), "Custom");
        assert!(err.to_string().contains("missing key"));
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
}
