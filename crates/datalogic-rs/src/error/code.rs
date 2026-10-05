//! [`ErrorCode`]: the kind of an [`Error`](super::Error) without its
//! payload.

use std::fmt;
use std::str::FromStr;

use super::ErrorKind;

/// What kind of error an [`Error`](super::Error) is, without the details:
/// one variant per [`ErrorKind`] variant, `Copy`, comparable and hashable.
/// Read it with [`Error::code`](super::Error::code).
///
/// Every variant exists in every build, whatever features are enabled, and
/// [`Self::as_str`] spells it the way [`Error::tag`](super::Error::tag)
/// always has, so a host can switch on the code instead of comparing tag
/// strings, and the bindings carry the same names on the wire.
///
/// `#[non_exhaustive]`: a later release may add kinds, so a `match` needs
/// a wildcard arm.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
#[non_exhaustive]
pub enum ErrorCode {
    /// [`ErrorKind::InvalidOperator`].
    InvalidOperator,
    /// [`ErrorKind::InvalidArguments`].
    InvalidArguments,
    /// [`ErrorKind::VariableNotFound`].
    VariableNotFound,
    /// [`ErrorKind::InvalidContextLevel`].
    InvalidContextLevel,
    /// [`ErrorKind::TypeError`].
    TypeError,
    /// [`ErrorKind::ArithmeticError`].
    ArithmeticError,
    /// [`ErrorKind::Custom`].
    Custom,
    /// [`ErrorKind::ParseError`].
    ParseError,
    /// [`ErrorKind::Thrown`].
    Thrown,
    /// [`ErrorKind::FormatError`].
    FormatError,
    /// [`ErrorKind::IndexOutOfBounds`].
    IndexOutOfBounds,
    /// [`ErrorKind::ConfigurationError`].
    ConfigurationError,
    /// [`ErrorKind::BudgetExceeded`].
    BudgetExceeded,
}

impl ErrorCode {
    /// Every code, in declaration order.
    pub const ALL: &'static [ErrorCode] = &[
        ErrorCode::InvalidOperator,
        ErrorCode::InvalidArguments,
        ErrorCode::VariableNotFound,
        ErrorCode::InvalidContextLevel,
        ErrorCode::TypeError,
        ErrorCode::ArithmeticError,
        ErrorCode::Custom,
        ErrorCode::ParseError,
        ErrorCode::Thrown,
        ErrorCode::FormatError,
        ErrorCode::IndexOutOfBounds,
        ErrorCode::ConfigurationError,
        ErrorCode::BudgetExceeded,
    ];

    /// The code's name, which is also the variant's name and
    /// [`Error::tag`](super::Error::tag). Stable across releases.
    pub const fn as_str(self) -> &'static str {
        match self {
            ErrorCode::InvalidOperator => "InvalidOperator",
            ErrorCode::InvalidArguments => "InvalidArguments",
            ErrorCode::VariableNotFound => "VariableNotFound",
            ErrorCode::InvalidContextLevel => "InvalidContextLevel",
            ErrorCode::TypeError => "TypeError",
            ErrorCode::ArithmeticError => "ArithmeticError",
            ErrorCode::Custom => "Custom",
            ErrorCode::ParseError => "ParseError",
            ErrorCode::Thrown => "Thrown",
            ErrorCode::FormatError => "FormatError",
            ErrorCode::IndexOutOfBounds => "IndexOutOfBounds",
            ErrorCode::ConfigurationError => "ConfigurationError",
            ErrorCode::BudgetExceeded => "BudgetExceeded",
        }
    }
}

impl fmt::Display for ErrorCode {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

/// The name is not one of [`ErrorCode::ALL`]'s [`as_str`](ErrorCode::as_str).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UnknownErrorCode;

impl fmt::Display for UnknownErrorCode {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("unknown error code")
    }
}

impl std::error::Error for UnknownErrorCode {}

impl FromStr for ErrorCode {
    type Err = UnknownErrorCode;

    /// Parse a code from its exact (case-sensitive) name.
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        ErrorCode::ALL
            .iter()
            .copied()
            .find(|code| code.as_str() == s)
            .ok_or(UnknownErrorCode)
    }
}

impl ErrorKind {
    /// This kind's [`ErrorCode`].
    pub const fn code(&self) -> ErrorCode {
        match self {
            ErrorKind::InvalidOperator(_) => ErrorCode::InvalidOperator,
            ErrorKind::InvalidArguments(_) => ErrorCode::InvalidArguments,
            ErrorKind::VariableNotFound(_) => ErrorCode::VariableNotFound,
            ErrorKind::InvalidContextLevel(_) => ErrorCode::InvalidContextLevel,
            ErrorKind::TypeError(_) => ErrorCode::TypeError,
            ErrorKind::ArithmeticError(_) => ErrorCode::ArithmeticError,
            ErrorKind::Custom(_) => ErrorCode::Custom,
            ErrorKind::ParseError(_) => ErrorCode::ParseError,
            ErrorKind::Thrown(_) => ErrorCode::Thrown,
            ErrorKind::FormatError(_) => ErrorCode::FormatError,
            ErrorKind::IndexOutOfBounds { .. } => ErrorCode::IndexOutOfBounds,
            ErrorKind::ConfigurationError(_) => ErrorCode::ConfigurationError,
            ErrorKind::BudgetExceeded { .. } => ErrorCode::BudgetExceeded,
        }
    }
}
