//! Configuration module for customizable Engine behavior
//!
//! This module provides configuration options to customize the evaluation behavior
//! of the Engine engine while maintaining backward compatibility.

use datavalue::OwnedDataValue;
use std::sync::Arc;

/// Main configuration structure for Engine evaluation behavior
#[derive(Clone, Debug)]
pub struct EvaluationConfig {
    /// How to handle NaN (Not a Number) in arithmetic operations
    pub arithmetic_nan_handling: NanHandling,

    /// How to handle division by zero
    pub division_by_zero: DivisionByZeroHandling,

    /// Whether to throw errors for incompatible types in loose equality
    pub loose_equality_errors: bool,

    /// How to evaluate truthiness of values
    pub truthy_evaluator: TruthyEvaluator,

    /// Configuration for numeric coercion behavior
    pub numeric_coercion: NumericCoercionConfig,
}

/// Defines how to handle NaN (Not a Number) scenarios in arithmetic operations
#[derive(Clone, Debug, PartialEq)]
pub enum NanHandling {
    /// Throw an error when encountering non-numeric values (default)
    ThrowError,
    /// Ignore non-numeric values and continue with remaining values
    IgnoreValue,
    /// Treat non-numeric values as zero
    CoerceToZero,
    /// Return null when encountering non-numeric values
    ReturnNull,
}

/// Defines how to handle division by zero
#[derive(Clone, Debug, PartialEq)]
pub enum DivisionByZeroHandling {
    /// Return f64::MAX or f64::MIN based on sign (default)
    ReturnBounds,
    /// Throw an error
    ThrowError,
    /// Return null
    ReturnNull,
    /// Return infinity (positive or negative based on dividend sign)
    ReturnInfinity,
}

/// Defines how to evaluate truthiness of values
#[derive(Clone)]
pub enum TruthyEvaluator {
    /// JavaScript-style truthiness (default)
    /// - false: null, false, 0, NaN, "", empty array, empty object
    /// - true: everything else
    JavaScript,

    /// Python-style truthiness
    /// - false: None/null, False, 0, 0.0, "", empty collections
    /// - true: everything else
    Python,

    /// Strict boolean truthiness
    /// - false: null, false
    /// - true: everything else
    StrictBoolean,

    /// Custom truthiness evaluator. Receives the value as an
    /// [`OwnedDataValue`] — the canonical v5 owned value type — so the
    /// callback works without enabling `serde_json` interop.
    ///
    /// Note: this variant cannot participate in `PartialEq` or in a
    /// derived [`Debug`] (the closure is opaque). The hand-rolled `Debug`
    /// impl prints `Custom(<fn>)` so the surrounding [`EvaluationConfig`]
    /// stays debug-printable.
    Custom(Arc<dyn Fn(&OwnedDataValue) -> bool + Send + Sync>),
}

impl std::fmt::Debug for TruthyEvaluator {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::JavaScript => f.write_str("JavaScript"),
            Self::Python => f.write_str("Python"),
            Self::StrictBoolean => f.write_str("StrictBoolean"),
            Self::Custom(_) => f.write_str("Custom(<fn>)"),
        }
    }
}

/// Configuration for numeric coercion behavior
#[derive(Clone, Debug)]
pub struct NumericCoercionConfig {
    /// Convert empty strings to 0 (default: true)
    pub empty_string_to_zero: bool,

    /// Convert null to 0 (default: true)
    pub null_to_zero: bool,

    /// Convert booleans to numbers (true=1, false=0) (default: true)
    pub bool_to_number: bool,

    /// Only allow strict numeric parsing (no coercion) (default: false)
    pub strict_numeric: bool,

    /// Convert undefined/missing values to 0 (default: false)
    pub undefined_to_zero: bool,
}

impl Default for EvaluationConfig {
    fn default() -> Self {
        Self {
            arithmetic_nan_handling: NanHandling::ThrowError,
            division_by_zero: DivisionByZeroHandling::ReturnBounds,
            loose_equality_errors: true,
            truthy_evaluator: TruthyEvaluator::JavaScript,
            numeric_coercion: NumericCoercionConfig::default(),
        }
    }
}

impl Default for NumericCoercionConfig {
    fn default() -> Self {
        Self {
            empty_string_to_zero: true,
            null_to_zero: true,
            bool_to_number: true,
            strict_numeric: false,
            undefined_to_zero: false,
        }
    }
}

impl EvaluationConfig {
    /// Create a new configuration with default settings
    pub fn new() -> Self {
        Self::default()
    }

    /// Create a configuration with safe arithmetic (ignores non-numeric values)
    pub fn safe_arithmetic() -> Self {
        Self {
            arithmetic_nan_handling: NanHandling::IgnoreValue,
            division_by_zero: DivisionByZeroHandling::ReturnNull,
            loose_equality_errors: false,
            ..Default::default()
        }
    }

    /// Create a configuration with strict behavior (more errors)
    pub fn strict() -> Self {
        Self {
            arithmetic_nan_handling: NanHandling::ThrowError,
            division_by_zero: DivisionByZeroHandling::ThrowError,
            loose_equality_errors: true,
            numeric_coercion: NumericCoercionConfig {
                empty_string_to_zero: false,
                null_to_zero: false,
                bool_to_number: false,
                strict_numeric: true,
                undefined_to_zero: false,
            },
            ..Default::default()
        }
    }
}
