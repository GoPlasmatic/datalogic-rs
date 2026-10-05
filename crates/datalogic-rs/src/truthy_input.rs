//! Input adapter for [`crate::Engine::truthy_of`]: the engine's configured
//! truthiness for a value the caller already holds, in whichever
//! representation it holds it.

use datavalue::OwnedDataValue;

use crate::Engine;
use crate::arena::DataValue;

mod sealed {
    pub trait Sealed {}
}

/// A value [`Engine::truthy_of`] can judge. **Sealed**: the supported
/// shapes are `&DataValue`, `&OwnedDataValue`, `&ParsedData` and, with the
/// `serde_json` feature, `&serde_json::Value`.
///
/// Truthiness reads only the top of a value (an array is truthy when it
/// is non-empty, whatever it holds), so no shape is converted, except for
/// a [`crate::TruthyEvaluator::Custom`] evaluator, which receives an
/// [`OwnedDataValue`] and so gets one built from a `serde_json::Value`.
pub trait TruthyInput: sealed::Sealed {
    #[doc(hidden)]
    fn truthy_with(self, engine: &Engine) -> bool;
}

impl sealed::Sealed for &DataValue<'_> {}
impl TruthyInput for &DataValue<'_> {
    #[inline]
    fn truthy_with(self, engine: &Engine) -> bool {
        crate::arena::truthy_arena(self, engine)
    }
}

impl sealed::Sealed for &OwnedDataValue {}
impl TruthyInput for &OwnedDataValue {
    #[inline]
    fn truthy_with(self, engine: &Engine) -> bool {
        crate::operators::truthy::truthy_owned(self, engine)
    }
}

impl sealed::Sealed for &crate::ParsedData {}
impl TruthyInput for &crate::ParsedData {
    #[inline]
    fn truthy_with(self, engine: &Engine) -> bool {
        crate::arena::truthy_arena(self.value(), engine)
    }
}

#[cfg(feature = "serde_json")]
impl sealed::Sealed for &serde_json::Value {}
#[cfg(feature = "serde_json")]
impl TruthyInput for &serde_json::Value {
    /// Mirrors `truthy_arena` / `truthy_owned` on the serde shape;
    /// `tests/truthy_of_test.rs` holds the three in lockstep against the
    /// engine's own `!!`.
    fn truthy_with(self, engine: &Engine) -> bool {
        use crate::config::TruthyEvaluator;
        use serde_json::Value;
        match &engine.config().truthy_evaluator {
            // A JSON number is never NaN, the one value on which Python
            // and JavaScript truthiness differ.
            TruthyEvaluator::JavaScript | TruthyEvaluator::Python => match self {
                Value::Null => false,
                Value::Bool(b) => *b,
                // No non-zero integer converts to 0.0, so one comparison
                // covers the i64, u64 and f64 forms.
                Value::Number(n) => n.as_f64().is_some_and(|f| f != 0.0),
                Value::String(s) => !s.is_empty(),
                Value::Array(items) => !items.is_empty(),
                Value::Object(fields) => !fields.is_empty(),
            },
            TruthyEvaluator::StrictBoolean => match self {
                Value::Null => false,
                Value::Bool(b) => *b,
                _ => true,
            },
            TruthyEvaluator::Custom(f) => f(&crate::serde_bridge::owned_from_serde(self)),
        }
    }
}
