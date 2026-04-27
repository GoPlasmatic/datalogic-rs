use datavalue::OwnedDataValue;
use serde::ser::{Serialize, SerializeMap, Serializer};
use std::fmt;

/// Error type for DataLogic operations
#[derive(Debug, Clone)]
pub enum Error {
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

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Error::InvalidOperator(op) => write!(f, "Invalid operator: {}", op),
            Error::InvalidArguments(msg) => write!(f, "Invalid arguments: {}", msg),
            Error::VariableNotFound(var) => write!(f, "Variable not found: {}", var),
            Error::InvalidContextLevel(level) => write!(f, "Invalid context level: {}", level),
            Error::TypeError(msg) => write!(f, "Type error: {}", msg),
            Error::ArithmeticError(msg) => write!(f, "Arithmetic error: {}", msg),
            Error::Custom(msg) => write!(f, "{}", msg),
            Error::ParseError(msg) => write!(f, "Parse error: {}", msg),
            Error::Thrown(val) => {
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
            Error::FormatError(msg) => write!(f, "Format error: {}", msg),
            Error::IndexOutOfBounds { index, length } => {
                write!(
                    f,
                    "Index {} out of bounds for array of length {}",
                    index, length
                )
            }
            Error::ConfigurationError(msg) => write!(f, "Configuration error: {}", msg),
        }
    }
}

impl std::error::Error for Error {}

#[cfg(feature = "compat")]
impl From<serde_json::Error> for Error {
    fn from(err: serde_json::Error) -> Self {
        Error::ParseError(err.to_string())
    }
}

impl From<datavalue::ParseError> for Error {
    fn from(err: datavalue::ParseError) -> Self {
        Error::ParseError(format!("{:?}", err))
    }
}

impl Error {
    /// Stable string tag for the error kind. Used in JSON serialization and
    /// stable across releases — TS consumers can match on it.
    fn tag(&self) -> &'static str {
        match self {
            Error::InvalidOperator(_) => "InvalidOperator",
            Error::InvalidArguments(_) => "InvalidArguments",
            Error::VariableNotFound(_) => "VariableNotFound",
            Error::InvalidContextLevel(_) => "InvalidContextLevel",
            Error::TypeError(_) => "TypeError",
            Error::ArithmeticError(_) => "ArithmeticError",
            Error::Custom(_) => "Custom",
            Error::ParseError(_) => "ParseError",
            Error::Thrown(_) => "Thrown",
            Error::FormatError(_) => "FormatError",
            Error::IndexOutOfBounds { .. } => "IndexOutOfBounds",
            Error::ConfigurationError(_) => "ConfigurationError",
        }
    }

    /// Writes the `type`, `message`, and any variant-specific extra fields
    /// into an existing `SerializeMap`. Shared by `Error` and
    /// `StructuredError` so the JSON shape stays in one place.
    fn serialize_fields<M: SerializeMap>(&self, map: &mut M) -> Result<(), M::Error> {
        map.serialize_entry("type", self.tag())?;
        map.serialize_entry("message", &self.to_string())?;
        match self {
            Error::VariableNotFound(name) => map.serialize_entry("variable", name),
            Error::InvalidContextLevel(level) => map.serialize_entry("level", level),
            Error::Thrown(value) => map.serialize_entry("thrown", value),
            Error::IndexOutOfBounds { index, length } => {
                map.serialize_entry("index", index)?;
                map.serialize_entry("length", length)
            }
            _ => Ok(()),
        }
    }
}

impl Serialize for Error {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        // Shape: { "type": <tag>, "message": <Display>, ...variant-specific extras }
        let mut map = serializer.serialize_map(None)?;
        self.serialize_fields(&mut map)?;
        map.end()
    }
}

/// An `Error` paired with optional contextual metadata about where it
/// occurred. Produced at the WASM/engine boundary for structured consumption
/// by non-Rust callers (e.g. the React debugger).
///
/// Serializes by flattening the inner `Error` — the JSON shape is:
/// `{"type": ..., "message": ..., ...extras, "operator": ..., "path": [...]}`.
///
/// The `path` field is a breadcrumb of [`CompiledNode`](crate::CompiledNode)
/// ids from the failing sub-expression up to the root, collected by the
/// dispatch hub during error unwinding. It is empty when the error came from
/// parse/compile (before evaluation began) or when tracing is running via
/// `compile_for_trace` where source-level ids are themselves synthetic.
#[derive(Debug, Clone, Default)]
pub struct StructuredError {
    pub error: Error,

    /// Name of the outermost operator that produced the error, when known.
    pub operator: Option<String>,

    /// Breadcrumb of node ids from the failure site toward the root.
    /// Empty means no evaluation path was collected.
    pub path: Vec<u32>,
}

impl Default for Error {
    fn default() -> Self {
        Error::Custom(String::new())
    }
}

impl Serialize for StructuredError {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        // Flatten the inner Error fields, then append operator + path when present.
        let mut map = serializer.serialize_map(None)?;
        self.error.serialize_fields(&mut map)?;
        if let Some(op) = &self.operator {
            map.serialize_entry("operator", op)?;
        }
        if !self.path.is_empty() {
            map.serialize_entry("path", &self.path)?;
        }
        map.end()
    }
}

impl StructuredError {
    /// Attach the given operator name and return self.
    pub fn with_operator(mut self, operator: impl Into<String>) -> Self {
        self.operator = Some(operator.into());
        self
    }

    /// Attach the given breadcrumb path and return self.
    pub fn with_path(mut self, path: Vec<u32>) -> Self {
        self.path = path;
        self
    }
}

impl From<Error> for StructuredError {
    fn from(error: Error) -> Self {
        StructuredError {
            error,
            operator: None,
            path: Vec::new(),
        }
    }
}

impl fmt::Display for StructuredError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match &self.operator {
            Some(op) => write!(f, "{} (in operator: {})", self.error, op),
            None => write!(f, "{}", self.error),
        }
    }
}

impl std::error::Error for StructuredError {}
