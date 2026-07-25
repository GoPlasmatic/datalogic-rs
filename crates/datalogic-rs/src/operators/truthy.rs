//! Truthiness on `OwnedDataValue` (compile-time literal form).
//!
//! The runtime arena-resident form has its own `truthy_arena` next to
//! the arena value helpers; this version is used by compile-time
//! constant folding and by the `Custom` truthy callback.

use crate::config::TruthyEvaluator;
use datavalue::{NumberValue, OwnedDataValue};

/// Truthiness for the compile-time owned form.
///
/// This is the folding-side twin of `truthy_arena`. The constant folder
/// reaches it through `optimize::helpers::is_truthy_literal`, so the two
/// must agree on every configuration: if they diverged, a literal
/// predicate folded at compile time would disagree with the same value
/// computed at runtime.
#[inline]
pub(crate) fn truthy_owned(value: &OwnedDataValue, engine: &crate::Engine) -> bool {
    match &engine.config().truthy_evaluator {
        TruthyEvaluator::JavaScript => truthy_js_owned(value),
        // Python treats `float('nan')` as truthy where JavaScript treats
        // `NaN` as falsy; everything else coincides.
        TruthyEvaluator::Python => match value {
            OwnedDataValue::Number(n) if n.is_nan() => true,
            _ => truthy_js_owned(value),
        },
        TruthyEvaluator::StrictBoolean => match value {
            OwnedDataValue::Null => false,
            OwnedDataValue::Bool(b) => *b,
            _ => true,
        },
        TruthyEvaluator::Custom(f) => f(value),
    }
}

#[inline]
fn truthy_js_owned(value: &OwnedDataValue) -> bool {
    match value {
        OwnedDataValue::Null => false,
        OwnedDataValue::Bool(b) => *b,
        OwnedDataValue::Number(NumberValue::Integer(i)) => *i != 0,
        OwnedDataValue::Number(NumberValue::Float(f)) => *f != 0.0 && !f.is_nan(),
        OwnedDataValue::String(s) => !s.is_empty(),
        OwnedDataValue::Array(items) => !items.is_empty(),
        OwnedDataValue::Object(pairs) => !pairs.is_empty(),
        #[cfg(feature = "datetime")]
        OwnedDataValue::DateTime(_) | OwnedDataValue::Duration(_) => true,
    }
}
