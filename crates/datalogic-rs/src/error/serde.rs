//! Serde + `Display` rendering for [`Error`], plus the `From` impls for
//! foreign parse errors. Split out so `mod.rs` stays focused on the struct
//! and its constructors.

use super::Error;
use super::kind::ErrorKind;
use serde::ser::{Serialize, SerializeMap, Serializer};
use std::borrow::Cow;
use std::fmt;

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        // Render the kind first, then optionally the operator context.
        write_kind_message(f, &self.kind)?;
        if let Some(op) = self.operator() {
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
            #[cfg(feature = "serde_json")]
            {
                let json = crate::serde_bridge::owned_to_serde(val);
                write!(f, "Thrown: {}", json)
            }
            #[cfg(not(feature = "serde_json"))]
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
    /// Returns the wrapped source error, but only for [`ErrorKind::Custom`].
    ///
    /// All other [`ErrorKind`] variants carry a flat `Cow<'static, str>`
    /// payload (or a structured value, in `Thrown` / `IndexOutOfBounds`)
    /// rather than a typed cause, so they have no `dyn Error` to chain to.
    /// To attach a typed source, wrap your error via [`Error::wrap`] —
    /// that produces an `ErrorKind::Custom` whose `source()` returns
    /// `Some(&original)` and whose `Display` matches the original.
    ///
    /// ```rust
    /// use datalogic_rs::Error;
    /// use std::error::Error as _;
    ///
    /// fn read_config() -> std::io::Result<String> {
    ///     Err(std::io::Error::other("disk fell off the cliff"))
    /// }
    ///
    /// let err = read_config().map_err(Error::wrap).unwrap_err();
    /// // The original io::Error survives the wrap and can be walked.
    /// let source = err.source().unwrap();
    /// assert!(source.to_string().contains("disk"));
    /// ```
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match &self.kind {
            ErrorKind::Custom(err) => Some(err.as_ref()),
            _ => None,
        }
    }
}

#[cfg(feature = "serde_json")]
#[cfg_attr(docsrs, doc(cfg(feature = "serde_json")))]
impl From<serde_json::Error> for Error {
    fn from(err: serde_json::Error) -> Self {
        Error::new(ErrorKind::ParseError(Cow::Owned(err.to_string())))
    }
}

impl From<datavalue::ParseError> for Error {
    fn from(err: datavalue::ParseError) -> Self {
        Error::new(ErrorKind::ParseError(Cow::Owned(err.to_string())))
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
        // { "type": <tag>, "message": <Display>, ...kind-extras, "operator"?, "node_ids"? }
        let mut map = serializer.serialize_map(None)?;
        map.serialize_entry("type", self.tag())?;
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
        if let Some(op) = self.operator() {
            map.serialize_entry("operator", op)?;
        }
        let ids = self.node_ids();
        if !ids.is_empty() {
            map.serialize_entry("node_ids", ids)?;
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
