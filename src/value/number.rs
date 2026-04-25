//! `NumberValue` — internal numeric type for the arena evaluation path.
//!
//! Replaces `serde_json::Number` inside `ArenaValue::Number`. Distinguishes
//! `Integer(i64)` from `Float(f64)` natively (serde_json::Number wraps an
//! opaque internal string) so integer arithmetic stays in i64 with overflow
//! checks instead of round-tripping through f64.
//!
//! Ported from v3.0.6 `src/value/number.rs` with minor adjustments for v4
//! (serde_json::Number conversion helpers, additional formatter for the
//! API boundary).

use serde_json::Number as SerdeNumber;
use std::cmp::Ordering;
use std::fmt;

/// Specialised numeric representation. Integers stay in i64 unless they
/// overflow during arithmetic, in which case the result falls back to f64.
#[derive(Debug, Clone, Copy)]
pub(crate) enum NumberValue {
    Integer(i64),
    Float(f64),
}

impl NumberValue {
    #[inline]
    pub(crate) fn from_i64(value: i64) -> Self {
        NumberValue::Integer(value)
    }

    /// Construct from an f64. Whole-valued floats within i64 range collapse
    /// to `Integer` so subsequent arithmetic uses the integer fast path.
    #[inline]
    pub(crate) fn from_f64(value: f64) -> Self {
        if value.fract() == 0.0
            && !value.is_nan()
            && !value.is_infinite()
            && value >= i64::MIN as f64
            && value <= i64::MAX as f64
        {
            NumberValue::Integer(value as i64)
        } else {
            NumberValue::Float(value)
        }
    }

    /// Construct from a `serde_json::Number`. Used at the API boundary when
    /// converting input `Value::Number` into the arena.
    #[inline]
    pub(crate) fn from_serde(n: &SerdeNumber) -> Self {
        if let Some(i) = n.as_i64() {
            NumberValue::Integer(i)
        } else if let Some(u) = n.as_u64() {
            // u64 values that fit in i64 stay as Integer; otherwise widen to f64.
            if u <= i64::MAX as u64 {
                NumberValue::Integer(u as i64)
            } else {
                NumberValue::Float(u as f64)
            }
        } else {
            // serde_json::Number always serializes f64; this branch is taken
            // for non-integer JSON numbers like 1.5.
            NumberValue::Float(n.as_f64().unwrap_or(0.0))
        }
    }

    /// Convert to `serde_json::Number` for the API boundary. Float NaN/Inf
    /// produce `None` because `serde_json::Number` rejects them; callers
    /// must handle that case (typically by emitting `Value::Null`).
    #[inline]
    pub(crate) fn to_serde(self) -> Option<SerdeNumber> {
        match self {
            NumberValue::Integer(i) => Some(SerdeNumber::from(i)),
            NumberValue::Float(f) => SerdeNumber::from_f64(f),
        }
    }

    #[inline]
    pub(crate) fn is_integer(&self) -> bool {
        matches!(self, NumberValue::Integer(_))
    }

    #[inline]
    pub(crate) fn as_i64(&self) -> Option<i64> {
        match *self {
            NumberValue::Integer(i) => Some(i),
            NumberValue::Float(f) => {
                if f.fract() == 0.0
                    && !f.is_nan()
                    && !f.is_infinite()
                    && f >= i64::MIN as f64
                    && f <= i64::MAX as f64
                {
                    Some(f as i64)
                } else {
                    None
                }
            }
        }
    }

    #[inline]
    pub(crate) fn as_f64(&self) -> f64 {
        match *self {
            NumberValue::Integer(i) => i as f64,
            NumberValue::Float(f) => f,
        }
    }

    #[inline]
    pub(crate) fn is_zero(&self) -> bool {
        match *self {
            NumberValue::Integer(i) => i == 0,
            NumberValue::Float(f) => f == 0.0,
        }
    }

    #[inline]
    pub(crate) fn is_nan(&self) -> bool {
        matches!(*self, NumberValue::Float(f) if f.is_nan())
    }

    /// Add. Integer-integer uses checked_add; on overflow falls back to f64.
    pub(crate) fn add(&self, other: &NumberValue) -> NumberValue {
        match (*self, *other) {
            (NumberValue::Integer(a), NumberValue::Integer(b)) => match a.checked_add(b) {
                Some(r) => NumberValue::Integer(r),
                None => NumberValue::Float(a as f64 + b as f64),
            },
            _ => NumberValue::from_f64(self.as_f64() + other.as_f64()),
        }
    }

    pub(crate) fn sub(&self, other: &NumberValue) -> NumberValue {
        match (*self, *other) {
            (NumberValue::Integer(a), NumberValue::Integer(b)) => match a.checked_sub(b) {
                Some(r) => NumberValue::Integer(r),
                None => NumberValue::Float(a as f64 - b as f64),
            },
            _ => NumberValue::from_f64(self.as_f64() - other.as_f64()),
        }
    }

    pub(crate) fn mul(&self, other: &NumberValue) -> NumberValue {
        match (*self, *other) {
            (NumberValue::Integer(a), NumberValue::Integer(b)) => match a.checked_mul(b) {
                Some(r) => NumberValue::Integer(r),
                None => NumberValue::Float(a as f64 * b as f64),
            },
            _ => NumberValue::from_f64(self.as_f64() * other.as_f64()),
        }
    }

    /// Divide. Returns `None` for division by zero — callers handle per
    /// `EvaluationConfig::division_by_zero` semantics.
    pub(crate) fn div(&self, other: &NumberValue) -> Option<NumberValue> {
        if other.is_zero() {
            return None;
        }
        match (*self, *other) {
            (NumberValue::Integer(a), NumberValue::Integer(b)) => {
                if a % b == 0 {
                    Some(NumberValue::Integer(a / b))
                } else {
                    Some(NumberValue::Float(a as f64 / b as f64))
                }
            }
            _ => Some(NumberValue::from_f64(self.as_f64() / other.as_f64())),
        }
    }

    /// Modulo. Returns `None` for division by zero — caller handles.
    pub(crate) fn rem(&self, other: &NumberValue) -> Option<NumberValue> {
        if other.is_zero() {
            return None;
        }
        match (*self, *other) {
            (NumberValue::Integer(a), NumberValue::Integer(b)) => Some(NumberValue::Integer(a % b)),
            _ => Some(NumberValue::from_f64(self.as_f64() % other.as_f64())),
        }
    }

    pub(crate) fn neg(&self) -> NumberValue {
        match *self {
            NumberValue::Integer(i) => match i.checked_neg() {
                Some(r) => NumberValue::Integer(r),
                None => NumberValue::Float(-(i as f64)),
            },
            NumberValue::Float(f) => NumberValue::Float(-f),
        }
    }

    pub(crate) fn abs(&self) -> NumberValue {
        match *self {
            NumberValue::Integer(i) => match i.checked_abs() {
                Some(r) => NumberValue::Integer(r),
                None => NumberValue::Float((i as f64).abs()),
            },
            NumberValue::Float(f) => NumberValue::Float(f.abs()),
        }
    }
}

impl PartialEq for NumberValue {
    fn eq(&self, other: &Self) -> bool {
        match (*self, *other) {
            (NumberValue::Integer(a), NumberValue::Integer(b)) => a == b,
            (NumberValue::Float(a), NumberValue::Float(b)) => a == b,
            (NumberValue::Integer(a), NumberValue::Float(b)) => (a as f64) == b,
            (NumberValue::Float(a), NumberValue::Integer(b)) => a == (b as f64),
        }
    }
}

impl PartialOrd for NumberValue {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        match (*self, *other) {
            (NumberValue::Integer(a), NumberValue::Integer(b)) => Some(a.cmp(&b)),
            (NumberValue::Float(a), NumberValue::Float(b)) => a.partial_cmp(&b),
            (NumberValue::Integer(a), NumberValue::Float(b)) => (a as f64).partial_cmp(&b),
            (NumberValue::Float(a), NumberValue::Integer(b)) => a.partial_cmp(&(b as f64)),
        }
    }
}

impl fmt::Display for NumberValue {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match *self {
            NumberValue::Integer(i) => write!(f, "{}", i),
            NumberValue::Float(fl) => {
                // Match serde_json::Number's f64 formatting: "1.5" not "1.5e0".
                if fl.is_nan() || fl.is_infinite() {
                    write!(f, "null")
                } else if fl.fract() == 0.0 {
                    write!(f, "{}.0", fl as i64)
                } else {
                    write!(f, "{}", fl)
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn from_f64_collapses_whole() {
        assert!(matches!(NumberValue::from_f64(42.0), NumberValue::Integer(42)));
        assert!(matches!(NumberValue::from_f64(-3.0), NumberValue::Integer(-3)));
        assert!(matches!(NumberValue::from_f64(1.5), NumberValue::Float(_)));
    }

    #[test]
    fn from_f64_rejects_nan_inf_for_int_path() {
        assert!(matches!(NumberValue::from_f64(f64::NAN), NumberValue::Float(_)));
        assert!(matches!(NumberValue::from_f64(f64::INFINITY), NumberValue::Float(_)));
    }

    #[test]
    fn add_overflow_falls_to_float() {
        let a = NumberValue::Integer(i64::MAX);
        let b = NumberValue::Integer(1);
        assert!(matches!(a.add(&b), NumberValue::Float(_)));
    }

    #[test]
    fn add_no_overflow_stays_int() {
        let a = NumberValue::Integer(2);
        let b = NumberValue::Integer(3);
        assert!(matches!(a.add(&b), NumberValue::Integer(5)));
    }

    #[test]
    fn div_zero_returns_none() {
        let a = NumberValue::Integer(1);
        let z = NumberValue::Integer(0);
        assert!(a.div(&z).is_none());
        let zf = NumberValue::Float(0.0);
        assert!(a.div(&zf).is_none());
    }

    #[test]
    fn div_int_int_exact_stays_int() {
        let a = NumberValue::Integer(10);
        let b = NumberValue::Integer(2);
        assert!(matches!(a.div(&b).unwrap(), NumberValue::Integer(5)));
    }

    #[test]
    fn div_int_int_inexact_promotes_float() {
        let a = NumberValue::Integer(7);
        let b = NumberValue::Integer(2);
        assert!(matches!(a.div(&b).unwrap(), NumberValue::Float(_)));
    }

    #[test]
    fn cross_type_eq_and_ord() {
        let i = NumberValue::Integer(5);
        let f = NumberValue::Float(5.0);
        assert_eq!(i, f);
        assert_eq!(i.partial_cmp(&f), Some(Ordering::Equal));

        let f2 = NumberValue::Float(5.5);
        assert_eq!(i.partial_cmp(&f2), Some(Ordering::Less));
    }

    #[test]
    fn serde_round_trip() {
        let n = SerdeNumber::from(42);
        let nv = NumberValue::from_serde(&n);
        assert!(matches!(nv, NumberValue::Integer(42)));
        let back = nv.to_serde().unwrap();
        assert_eq!(back, n);

        let f = SerdeNumber::from_f64(1.5).unwrap();
        let nv = NumberValue::from_serde(&f);
        assert!(matches!(nv, NumberValue::Float(_)));
        assert_eq!(nv.as_f64(), 1.5);
    }

    #[test]
    fn neg_overflow_falls_to_float() {
        let a = NumberValue::Integer(i64::MIN);
        assert!(matches!(a.neg(), NumberValue::Float(_)));
    }
}
