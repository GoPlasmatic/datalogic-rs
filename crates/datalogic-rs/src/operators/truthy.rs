//! Truthiness, written once for every value representation.
//!
//! The engine judges truthiness on three shapes: the arena `DataValue`
//! (evaluation), the owned `OwnedDataValue` (constant folding and
//! `Engine::truthy_of`), and `serde_json::Value` (`Engine::truthy_of`).
//! Each used to carry its own copy of the rules, kept in step by hand; a
//! drift between the arena and owned copies would make a folded predicate
//! disagree with the same value computed at runtime.
//!
//! Now each shape only says what truthiness reads of it ([`Truthy::shape`]:
//! null, a boolean, a number's zero/NaN-ness, a sized value's emptiness),
//! and [`truthy_by`] applies the configured [`TruthyEvaluator`] to that.

use crate::config::TruthyEvaluator;
use datavalue::OwnedDataValue;

/// What truthiness reads of a value: its top level only.
#[derive(Clone, Copy)]
pub(crate) enum Shape {
    Null,
    Bool(bool),
    Number {
        zero: bool,
        nan: bool,
    },
    /// A string, array, object or tensor: falsy when empty under the
    /// JavaScript rules. A tensor's size is its element count, which is 1
    /// for a 0-d tensor and 0 as soon as any axis is 0.
    Sized {
        empty: bool,
    },
    /// A value with no falsy form (a datetime or duration).
    #[cfg_attr(not(feature = "datetime"), allow(dead_code))]
    Always,
}

/// A value representation [`truthy_by`] can judge.
pub(crate) trait Truthy {
    /// What truthiness reads of `self`.
    fn shape(&self) -> Shape;

    /// Run a [`TruthyEvaluator::Custom`] callback on `self`, which takes an
    /// owned value.
    fn custom(&self, f: &(dyn Fn(&OwnedDataValue) -> bool + Send + Sync)) -> bool;
}

/// `value`'s truthiness under `evaluator`. The single implementation of
/// the rules; `truthy_arena`, [`truthy_owned`] and `Engine::truthy_of`
/// all come here.
///
/// `#[inline(always)]`: it sits inside the per-item path of every
/// quantifier and filter, and once inlined the shape folds away into the
/// direct match it replaces.
#[inline(always)]
pub(crate) fn truthy_by<V: Truthy + ?Sized>(value: &V, evaluator: &TruthyEvaluator) -> bool {
    match evaluator {
        TruthyEvaluator::JavaScript => js(value.shape()),
        // Python differs from JavaScript on exactly one value: `NaN`.
        // `float('nan')` is truthy in Python, falsy in JavaScript. Every
        // other rule (0, "", empty collections) coincides.
        TruthyEvaluator::Python => match value.shape() {
            Shape::Number { nan: true, .. } => true,
            shape => js(shape),
        },
        TruthyEvaluator::StrictBoolean => match value.shape() {
            Shape::Null => false,
            Shape::Bool(b) => b,
            _ => true,
        },
        TruthyEvaluator::Custom(f) => value.custom(&**f),
    }
}

/// The JavaScript rules.
#[inline(always)]
fn js(shape: Shape) -> bool {
    match shape {
        Shape::Null => false,
        Shape::Bool(b) => b,
        Shape::Number { zero, nan } => !zero && !nan,
        Shape::Sized { empty } => !empty,
        Shape::Always => true,
    }
}

impl Truthy for crate::arena::DataValue<'_> {
    #[inline(always)]
    fn shape(&self) -> Shape {
        use crate::arena::DataValue;
        match self {
            DataValue::Null => Shape::Null,
            DataValue::Bool(b) => Shape::Bool(*b),
            DataValue::Number(n) => Shape::Number {
                zero: n.is_zero(),
                nan: n.is_nan(),
            },
            DataValue::String(s) => Shape::Sized {
                empty: s.is_empty(),
            },
            DataValue::Array(items) => Shape::Sized {
                empty: items.is_empty(),
            },
            DataValue::Object(pairs) => Shape::Sized {
                empty: pairs.is_empty(),
            },
            #[cfg(feature = "datetime")]
            DataValue::DateTime(_) | DataValue::Duration(_) => Shape::Always,
            #[cfg(feature = "tensor")]
            DataValue::Tensor(t) => Shape::Sized {
                empty: t.numel() == 0,
            },
        }
    }

    #[inline]
    fn custom(&self, f: &(dyn Fn(&OwnedDataValue) -> bool + Send + Sync)) -> bool {
        f(&self.to_owned())
    }
}

impl Truthy for OwnedDataValue {
    #[inline(always)]
    fn shape(&self) -> Shape {
        match self {
            OwnedDataValue::Null => Shape::Null,
            OwnedDataValue::Bool(b) => Shape::Bool(*b),
            OwnedDataValue::Number(n) => Shape::Number {
                zero: n.is_zero(),
                nan: n.is_nan(),
            },
            OwnedDataValue::String(s) => Shape::Sized {
                empty: s.is_empty(),
            },
            OwnedDataValue::Array(items) => Shape::Sized {
                empty: items.is_empty(),
            },
            OwnedDataValue::Object(pairs) => Shape::Sized {
                empty: pairs.is_empty(),
            },
            #[cfg(feature = "datetime")]
            OwnedDataValue::DateTime(_) | OwnedDataValue::Duration(_) => Shape::Always,
            #[cfg(feature = "tensor")]
            OwnedDataValue::Tensor(t) => Shape::Sized {
                empty: t.numel() == 0,
            },
        }
    }

    #[inline]
    fn custom(&self, f: &(dyn Fn(&OwnedDataValue) -> bool + Send + Sync)) -> bool {
        f(self)
    }
}

#[cfg(feature = "serde_json")]
impl Truthy for serde_json::Value {
    #[inline]
    fn shape(&self) -> Shape {
        use serde_json::Value;
        match self {
            Value::Null => Shape::Null,
            Value::Bool(b) => Shape::Bool(*b),
            // A JSON number is never NaN. No non-zero integer converts to
            // 0.0, so one comparison covers the i64, u64 and f64 forms; a
            // number with no f64 form counts as zero.
            Value::Number(n) => Shape::Number {
                zero: !n.as_f64().is_some_and(|f| f != 0.0),
                nan: false,
            },
            Value::String(s) => Shape::Sized {
                empty: s.is_empty(),
            },
            Value::Array(items) => Shape::Sized {
                empty: items.is_empty(),
            },
            // A tagged tensor object is an ordinary object here.
            Value::Object(fields) => Shape::Sized {
                empty: fields.is_empty(),
            },
        }
    }

    #[inline]
    fn custom(&self, f: &(dyn Fn(&OwnedDataValue) -> bool + Send + Sync)) -> bool {
        f(&crate::serde_bridge::owned_from_serde(self))
    }
}

/// Truthiness for the compile-time owned form, under `engine`'s
/// configured evaluator. The constant folder reaches it through
/// `optimize::helpers::is_truthy_literal`.
#[inline]
pub(crate) fn truthy_owned(value: &OwnedDataValue, engine: &crate::Engine) -> bool {
    truthy_by(value, &engine.config().truthy_evaluator)
}
