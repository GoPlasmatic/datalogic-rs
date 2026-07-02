use datavalue::OwnedDataValue;
use std::borrow::Cow;
use std::sync::Arc;

/// Trait-object alias for the source carried by [`ErrorKind::Custom`].
/// Reference-counted so [`ErrorKind`] stays cheap to clone, and bounded
/// so a single `crate::Error` value can be sent across threads.
pub type CustomErrorSource = Arc<dyn std::error::Error + Send + Sync + 'static>;

/// Discriminant for [`crate::Error`]. Stable variant tags are exposed via
/// [`crate::Error::tag`] for matching across releases.
#[non_exhaustive]
#[derive(Debug, Clone)]
pub enum ErrorKind {
    /// Invalid operator name
    InvalidOperator(Cow<'static, str>),
    /// Invalid arguments for an operator
    InvalidArguments(Cow<'static, str>),
    /// Variable not found in context
    VariableNotFound(Cow<'static, str>),
    /// Invalid context level access
    InvalidContextLevel(isize),
    /// Type conversion/coercion error
    TypeError(Cow<'static, str>),
    /// Arithmetic error (division by zero, overflow, etc.)
    ArithmeticError(Cow<'static, str>),
    /// Custom error for extensions. Carries the underlying typed error so
    /// callers can walk the source chain via [`std::error::Error::source`].
    /// Constructed via [`crate::Error::custom_message`] (string-only) or
    /// [`crate::Error::wrap`] (any `std::error::Error + Send + Sync + 'static`).
    Custom(CustomErrorSource),
    /// JSON parsing/serialization error
    ParseError(Cow<'static, str>),
    /// Thrown error from throw operator
    Thrown(OwnedDataValue),
    /// Invalid format string or pattern
    FormatError(Cow<'static, str>),
    /// Index out of bounds for array operations
    IndexOutOfBounds {
        /// The out-of-range index that was requested.
        index: isize,
        /// The length of the array being indexed.
        length: usize,
    },
    /// Invalid operator configuration
    ConfigurationError(Cow<'static, str>),
}
