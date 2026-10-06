//! Operator implementations for the Engine rule engine.
//!
//! This module contains all built-in operator implementations organized by category.
//! Each operator follows a consistent pattern: a function that takes compiled arguments,
//! a context stack, and the engine reference, returning a `Result<Value>`.
//!
//! # Operator → required feature
//!
//! The default build (`features = []`) carries the JSONLogic baseline.
//! Extra operators live behind opt-in features; rules that use them
//! against an engine compiled without the feature error out at compile
//! time as `InvalidOperator("…")`.
//!
//! | Operator(s) | Required feature |
//! |---|---|
//! | `var`, `val` | *baseline* (always available) |
//! | `==`, `===`, `!=`, `!==`, `>`, `>=`, `<`, `<=` | *baseline* |
//! | `and`, `or`, `!`, `!!`, `if`, `?:` | *baseline* |
//! | `+`, `-`, `*`, `/`, `%`, `min`, `max` | *baseline* |
//! | `cat`, `substr`, `in` | *baseline* |
//! | `map`, `filter`, `reduce`, `merge`, `all`, `some`, `none` | *baseline* |
//! | `missing`, `missing_some` | *baseline* |
//! | `length`, `starts_with`, `ends_with`, `upper`, `lower`, `trim`, `split` | `ext-string` |
//! | `sort`, `slice`, `group_by`, `distinct` | `ext-array` |
//! | `keys`, `values`, `entries` | `ext-object` |
//! | `abs`, `ceil`, `floor` | `ext-math` |
//! | `exists`, `??`, `switch`/`match`, `type` | `ext-control` |
//! | `try`, `throw` | `error-handling` |
//! | `datetime`, `timestamp`, `parse_date`, `format_date`, `date_diff`, `now` | `datetime` |
//! | `fractional`, `sem_ver` ([flagd-compat][flagd]) | `flagd` |
//!
//! [flagd]: https://flagd.dev/reference/custom-operations/
//!
//! # Operator Categories
//!
//! - **Variable Access**: `var`, `val`, `exists` - Access data from context
//! - **Comparison**: `==`, `===`, `!=`, `!==`, `>`, `>=`, `<`, `<=` - Compare values
//! - **Logical**: `and`, `or`, `!`, `!!` - Boolean logic operations
//! - **Control Flow**: `if`, `?:`, `??` - Conditional evaluation
//! - **Arithmetic**: `+`, `-`, `*`, `/`, `%`, `min`, `max`, `abs`, `ceil`, `floor`
//! - **String**: `cat`, `substr`, `in`, `length`, `starts_with`, `ends_with`, `upper`, `lower`, `trim`, `split`
//! - **Array**: `map`, `filter`, `reduce`, `merge`, `all`, `some`, `none`, `sort`, `slice`
//! - **DateTime**: `datetime`, `timestamp`, `parse_date`, `format_date`, `date_diff`, `now`
//! - **Error Handling**: `try`, `throw` - Exception-like error handling
//! - **Type**: `type` - Runtime type inspection
//! - **Missing**: `missing`, `missing_some` - Check for missing fields
//! - **flagd-compat**: `fractional`, `sem_ver` — feature-flagging operators
//!   from the [OpenFeature flagd in-process provider
//!   spec](https://flagd.dev/reference/custom-operations/), implemented to
//!   match the canonical Go evaluator byte-for-byte. Gated on `flagd`.
//!
//! # Dispatch Mechanism
//!
//! Every built-in operator is one row of the operator table in
//! [`table`]: the row names the operator, points at its implementation,
//! and declares its facts ([`meta::OpMeta`]). The `OpCode` enum, name
//! lookup, dispatch arms and every optimizer classification are generated
//! or derived from the rows. During compilation, operator names are
//! converted to `OpCode` variants for fast runtime dispatch without string
//! comparisons.

pub(crate) mod truthy;

// The operator table, the facts it declares, and the typed-row adapters.
pub(crate) mod eager;
pub(crate) mod extract;
pub(crate) mod info;
pub(crate) mod meta;
pub(crate) mod table;

// Core - always compiled
pub(crate) mod arithmetic;
pub(crate) mod array;
pub(crate) mod comparison;
pub(crate) mod control;
pub(crate) mod logical;
pub(crate) mod missing;
pub(crate) mod string;
pub(crate) mod variable;

// Feature-gated extended operators
#[cfg(feature = "datetime")]
pub(crate) mod datetime;
#[cfg(feature = "error-handling")]
pub(crate) mod error_handling;
#[cfg(feature = "flagd")]
pub(crate) mod flagd;
#[cfg(feature = "ext-control")]
pub(crate) mod inspect;
#[cfg(feature = "ext-object")]
pub(crate) mod object;
#[cfg(feature = "tensor")]
pub(crate) mod tensor;

#[cfg(test)]
mod table_tests;

/// The form a NaN failure surfaces as. 5.x keeps both: which one a site
/// raises is observable (`try` catches either, but the error kind and the
/// catch arm's payload differ), so unifying them is a v6 change.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum NanForm {
    /// `Thrown({"type": "NaN"})`: arithmetic, ordering, duration division.
    Thrown,
    /// `InvalidArguments("NaN")`: loose equality between operands it cannot
    /// compare, and a `slice` bound that is not an integer.
    InvalidArguments,
}

/// Every NaN error a built-in operator raises comes from here, so the
/// sites are in one list and a later unification is a one-line change.
/// The thrown form takes the deferred fast lane inside a `try` (see
/// [`crate::Error::nan_at`]); what a caller observes is the same.
#[inline]
pub(crate) fn nan_error(form: NanForm, ctx: &mut crate::arena::ContextStack<'_>) -> crate::Error {
    match form {
        NanForm::Thrown => crate::Error::nan_at(ctx),
        NanForm::InvalidArguments => crate::Error::invalid_arguments(crate::error::NAN_ERROR),
    }
}

/// The name of a value's type: what `type` reports for anything but a
/// string or a datetime / duration sentinel object (which it classifies
/// further), and what `throw` puts under `"type"` for a thrown scalar.
#[cfg_attr(
    not(any(feature = "ext-control", feature = "error-handling")),
    allow(dead_code)
)]
pub(crate) fn type_name(v: &crate::arena::DataValue<'_>) -> &'static str {
    use crate::arena::DataValue;
    match v {
        DataValue::Null => "null",
        DataValue::Bool(_) => "boolean",
        DataValue::Number(_) => "number",
        DataValue::String(_) => "string",
        DataValue::Array(_) => "array",
        DataValue::Object(_) => "object",
        #[cfg(feature = "datetime")]
        DataValue::DateTime(_) => "datetime",
        #[cfg(feature = "datetime")]
        DataValue::Duration(_) => "duration",
        #[cfg(feature = "tensor")]
        DataValue::Tensor(_) => "tensor",
    }
}

/// [`type_name`] for an owned value.
#[cfg_attr(not(feature = "error-handling"), allow(dead_code))]
pub(crate) fn owned_type_name(v: &datavalue::OwnedDataValue) -> &'static str {
    use datavalue::OwnedDataValue;
    match v {
        OwnedDataValue::Null => "null",
        OwnedDataValue::Bool(_) => "boolean",
        OwnedDataValue::Number(_) => "number",
        OwnedDataValue::String(_) => "string",
        OwnedDataValue::Array(_) => "array",
        OwnedDataValue::Object(_) => "object",
        #[cfg(feature = "datetime")]
        OwnedDataValue::DateTime(_) => "datetime",
        #[cfg(feature = "datetime")]
        OwnedDataValue::Duration(_) => "duration",
        #[cfg(feature = "tensor")]
        OwnedDataValue::Tensor(_) => "tensor",
    }
}
