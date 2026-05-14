//! Truthiness on `OwnedDataValue` (compile-time literal form).
//!
//! The runtime arena-resident form has its own `truthy_arena` next to
//! the arena value helpers; this version is used by compile-time
//! constant folding and by the `Custom` truthy callback.

use crate::config::TruthyEvaluator;
use datavalue::{NumberValue, OwnedDataValue};

#[inline]
pub(crate) fn truthy_owned(value: &OwnedDataValue, engine: &crate::Engine) -> bool {
    match &engine.config().truthy_evaluator {
        TruthyEvaluator::JavaScript | TruthyEvaluator::Python => truthy_js_owned(value),
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
