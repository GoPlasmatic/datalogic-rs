//! Efficient numeric value representation.
//!
//! This module provides a specialized representation for numeric values
//! to optimize memory usage based on the actual value.

use std::cmp::Ordering;
use std::fmt;

/// Specialized representation for numeric values to optimize memory usage.
///
/// This enum provides different representations for integers and floating-point
/// values, allowing for more efficient memory usage and operations.
#[derive(Debug, Clone, Copy)]
pub enum NumberValue {
    /// Integer value
    Integer(i64),

    /// Floating point value
    Float(f64),
}

impl NumberValue {
    /// Creates a new NumberValue from an i64.
    pub fn from_i64(value: i64) -> Self {
        NumberValue::Integer(value)
    }

    /// Creates a new NumberValue from an f64.
    pub fn from_f64(value: f64) -> Self {
        // Store integers as integers when possible
        if value.fract() == 0.0 && value >= i64::MIN as f64 && value <= i64::MAX as f64 {
            NumberValue::Integer(value as i64)
        } else {
            NumberValue::Float(value)
        }
    }

    /// Returns true if the value is an integer.
    pub fn is_integer(&self) -> bool {
        matches!(self, NumberValue::Integer(_))
    }

    /// Returns true if the value is a floating point.
    pub fn is_float(&self) -> bool {
        matches!(self, NumberValue::Float(_))
    }

    /// Returns the value as an i64, if possible.
    pub fn as_i64(&self) -> Option<i64> {
        match *self {
            NumberValue::Integer(i) => Some(i),
            NumberValue::Float(f) => {
                if f.fract() == 0.0 && f >= i64::MIN as f64 && f <= i64::MAX as f64 {
                    Some(f as i64)
                } else {
                    None
                }
            }
        }
    }

    /// Returns the value as an f64.
    pub fn as_f64(&self) -> f64 {
        match *self {
            NumberValue::Integer(i) => i as f64,
            NumberValue::Float(f) => f,
        }
    }

    /// Adds another NumberValue to this one.
    pub fn add(&self, other: &NumberValue) -> NumberValue {
        match (*self, *other) {
            (NumberValue::Integer(a), NumberValue::Integer(b)) => {
                // Check for overflow
                match a.checked_add(b) {
                    Some(result) => NumberValue::Integer(result),
                    None => NumberValue::Float(a as f64 + b as f64),
                }
            }
            _ => NumberValue::from_f64(self.as_f64() + other.as_f64()),
        }
    }

    /// Subtracts another NumberValue from this one.
    pub fn subtract(&self, other: &NumberValue) -> NumberValue {
        match (*self, *other) {
            (NumberValue::Integer(a), NumberValue::Integer(b)) => {
                // Check for overflow
                match a.checked_sub(b) {
                    Some(result) => NumberValue::Integer(result),
                    None => NumberValue::Float(a as f64 - b as f64),
                }
            }
            _ => NumberValue::from_f64(self.as_f64() - other.as_f64()),
        }
    }

    /// Multiplies this NumberValue by another.
    pub fn multiply(&self, other: &NumberValue) -> NumberValue {
        match (*self, *other) {
            (NumberValue::Integer(a), NumberValue::Integer(b)) => {
                // Check for overflow
                match a.checked_mul(b) {
                    Some(result) => NumberValue::Integer(result),
                    None => NumberValue::Float(a as f64 * b as f64),
                }
            }
            _ => NumberValue::from_f64(self.as_f64() * other.as_f64()),
        }
    }

    /// Divides this NumberValue by another.
    pub fn divide(&self, other: &NumberValue) -> Option<NumberValue> {
        let divisor = other.as_f64();
        if divisor == 0.0 {
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
            _ => Some(NumberValue::from_f64(self.as_f64() / divisor)),
        }
    }

    /// Returns the modulo of this NumberValue by another.
    pub fn modulo(&self, other: &NumberValue) -> Option<NumberValue> {
        let divisor = other.as_f64();
        if divisor == 0.0 {
            return None;
        }

        match (*self, *other) {
            (NumberValue::Integer(a), NumberValue::Integer(b)) => Some(NumberValue::Integer(a % b)),
            _ => Some(NumberValue::from_f64(self.as_f64() % divisor)),
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

impl Eq for NumberValue {}

impl PartialOrd for NumberValue {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        match (*self, *other) {
            (NumberValue::Integer(a), NumberValue::Integer(b)) => a.partial_cmp(&b),
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
            NumberValue::Float(fl) => write!(f, "{}", fl),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_number_creation() {
        let int = NumberValue::from_i64(42);
        let float = NumberValue::from_f64(3.14);
        let int_from_float = NumberValue::from_f64(42.0);

        assert!(int.is_integer());
        assert!(float.is_float());
        assert!(int_from_float.is_integer());

        assert_eq!(int.as_i64(), Some(42));
        assert_eq!(float.as_i64(), None);
        assert_eq!(int_from_float.as_i64(), Some(42));

        assert_eq!(int.as_f64(), 42.0);
        assert_eq!(float.as_f64(), 3.14);
    }

    #[test]
    fn test_number_operations() {
        let a = NumberValue::from_i64(5);
        let b = NumberValue::from_i64(3);
        let c = NumberValue::from_f64(2.5);

        assert_eq!(a.add(&b), NumberValue::from_i64(8));
        assert_eq!(a.subtract(&b), NumberValue::from_i64(2));
        assert_eq!(a.multiply(&b), NumberValue::from_i64(15));
        assert_eq!(a.divide(&b).unwrap(), NumberValue::from_f64(5.0 / 3.0));

        assert_eq!(a.add(&c), NumberValue::from_f64(7.5));
        assert_eq!(a.subtract(&c), NumberValue::from_f64(2.5));
        assert_eq!(a.multiply(&c), NumberValue::from_f64(12.5));
        assert_eq!(a.divide(&c).unwrap(), NumberValue::from_f64(2.0));
    }

    #[test]
    fn test_number_comparison() {
        let a = NumberValue::from_i64(5);
        let b = NumberValue::from_i64(3);
        let c = NumberValue::from_f64(5.0);
        let d = NumberValue::from_f64(3.5);

        assert!(a > b);
        assert!(a == c);
        assert!(a > d);
        assert!(d > b);
    }
}
