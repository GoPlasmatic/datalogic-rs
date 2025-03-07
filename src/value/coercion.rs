//! Value coercion utilities.
//!
//! This module provides traits and functions for coercing values between different types
//! according to JSONLogic rules.

use crate::arena::DataArena;
use crate::value::{DataValue, NumberValue};
use crate::logic::error::{LogicError, Result};
use std::fmt::Write;

/// A trait for value coercion operations.
pub trait ValueCoercion<'a> {
    /// Returns true if the value is considered null.
    fn is_null_value(&self) -> bool;
    
    /// Coerces the value to a boolean according to JSONLogic rules.
    fn coerce_to_bool(&self) -> bool;
    
    /// Coerces the value to a number according to JSONLogic rules.
    fn coerce_to_number(&self) -> Option<NumberValue>;
    
    /// Coerces the value to a string according to JSONLogic rules.
    fn coerce_to_string(&self, arena: &'a DataArena) -> DataValue<'a>;
    
    /// Appends the string representation of the value to the given string.
    fn coerce_append(&self, result: &mut String);
}

impl<'a> ValueCoercion<'a> for DataValue<'a> {
    fn is_null_value(&self) -> bool {
        matches!(self, DataValue::Null)
    }
    
    fn coerce_to_bool(&self) -> bool {
        match self {
            DataValue::Null => false,
            DataValue::Bool(b) => *b,
            DataValue::Number(n) => {
                match n {
                    NumberValue::Integer(i) => *i != 0,
                    NumberValue::Float(f) => *f != 0.0 && !f.is_nan(),
                }
            },
            DataValue::String(s) => !s.is_empty(),
            DataValue::Array(a) => !a.is_empty(),
            DataValue::Object(o) => !o.is_empty(),
        }
    }
    
    fn coerce_to_number(&self) -> Option<NumberValue> {
        match self {
            DataValue::Null => Some(NumberValue::Integer(0)),
            DataValue::Bool(b) => Some(NumberValue::Integer(if *b { 1 } else { 0 })),
            DataValue::Number(n) => Some(*n),
            DataValue::String(s) => {
                // Try to parse as integer first
                if let Ok(i) = s.parse::<i64>() {
                    Some(NumberValue::Integer(i))
                } else if let Ok(f) = s.parse::<f64>() {
                    Some(NumberValue::Float(f))
                } else {
                    None
                }
            },
            DataValue::Array(a) => {
                if a.is_empty() {
                    Some(NumberValue::Integer(0))
                } else if a.len() == 1 {
                    a[0].coerce_to_number()
                } else {
                    None
                }
            },
            DataValue::Object(_) => None,
        }
    }
    
    fn coerce_to_string(&self, arena: &'a DataArena) -> DataValue<'a> {
        match self {
            DataValue::Null => DataValue::String(arena.alloc_str("null")),
            DataValue::Bool(b) => DataValue::String(arena.alloc_str(if *b { "true" } else { "false" })),
            DataValue::Number(n) => DataValue::String(arena.alloc_str(&n.to_string())),
            DataValue::String(s) => DataValue::String(s),
            DataValue::Array(a) => {
                let mut result = String::new();
                for (i, v) in a.iter().enumerate() {
                    if i > 0 {
                        result.push(',');
                    }
                    v.coerce_append(&mut result);
                }
                DataValue::String(arena.alloc_str(&result))
            },
            DataValue::Object(_) => DataValue::String(arena.alloc_str("[object Object]")),
        }
    }
    
    fn coerce_append(&self, result: &mut String) {
        match self {
            DataValue::Null => result.push_str("null"),
            DataValue::Bool(b) => result.push_str(if *b { "true" } else { "false" }),
            DataValue::Number(n) => {
                match n {
                    NumberValue::Integer(i) => write!(result, "{}", i).unwrap(),
                    NumberValue::Float(f) => write!(result, "{}", f).unwrap(),
                }
            },
            DataValue::String(s) => result.push_str(s),
            DataValue::Array(_) => result.push_str("[array]"),
            DataValue::Object(_) => result.push_str("[object]"),
        }
    }
}

/// A trait for value comparison operations.
pub trait ValueComparison<'a> {
    /// Compares two values for strict equality.
    fn strict_equals(&self, other: &DataValue<'a>) -> bool;
    
    /// Compares two values for strict inequality.
    fn strict_not_equals(&self, other: &DataValue<'a>) -> bool;
    
    /// Compares two values for loose equality.
    fn equals(&self, other: &DataValue<'a>) -> bool;
    
    /// Compares two values for loose inequality.
    fn not_equals(&self, other: &DataValue<'a>) -> bool;
    
    /// Compares if this value is greater than another.
    fn greater_than(&self, other: &DataValue<'a>) -> Result<bool>;
    
    /// Compares if this value is greater than or equal to another.
    fn greater_than_equal(&self, other: &DataValue<'a>) -> Result<bool>;
    
    /// Compares if this value is less than another.
    fn less_than(&self, other: &DataValue<'a>) -> Result<bool>;
    
    /// Compares if this value is less than or equal to another.
    fn less_than_equal(&self, other: &DataValue<'a>) -> Result<bool>;
}

impl<'a> ValueComparison<'a> for DataValue<'a> {
    fn strict_equals(&self, other: &DataValue<'a>) -> bool {
        self == other
    }
    
    fn strict_not_equals(&self, other: &DataValue<'a>) -> bool {
        !self.strict_equals(other)
    }
    
    fn equals(&self, other: &DataValue<'a>) -> bool {
        match (self, other) {
            // Same types use strict equality
            (DataValue::Number(a), DataValue::Number(b)) => a == b,
            (DataValue::String(a), DataValue::String(b)) => a == b,
            (DataValue::Bool(a), DataValue::Bool(b)) => a == b,
            (DataValue::Null, DataValue::Null) => true,

            (DataValue::Number(a), DataValue::String(b)) => {
                if let Ok(num) = b.parse::<f64>() {
                    a.as_f64() == num
                } else {
                    false
                }
            },
            (DataValue::String(a), DataValue::Number(b)) => {
                if let Ok(num) = a.parse::<f64>() {
                    num == b.as_f64()
                } else {
                    false
                }
            },

            // Different types need coercion
            (DataValue::Null, DataValue::Bool(b)) => !b,
            (DataValue::Bool(a), DataValue::Null) => !a,
            
            // Arrays and objects use reference equality
            (DataValue::Array(_), DataValue::Array(_)) => self == other,
            (DataValue::Object(_), DataValue::Object(_)) => self == other,
            
            // All other combinations are false
            _ => false,
        }
    }
    
    fn not_equals(&self, other: &DataValue<'a>) -> bool {
        !self.equals(other)
    }
    
    fn greater_than(&self, other: &DataValue<'a>) -> Result<bool> {
        match (self, other) {
            (DataValue::Number(a), DataValue::Number(b)) => Ok(a.as_f64() > b.as_f64()),
            (DataValue::String(a), DataValue::String(b)) => Ok(*a > *b),
            
            // Coerce to numbers for comparison
            (a, b) => {
                let a_num = a.coerce_to_number().ok_or_else(|| 
                    LogicError::type_mismatch("number", a.type_name().to_string()))?;
                let b_num = b.coerce_to_number().ok_or_else(|| 
                    LogicError::type_mismatch("number", b.type_name().to_string()))?;
                
                Ok(a_num.as_f64() > b_num.as_f64())
            }
        }
    }
    
    fn greater_than_equal(&self, other: &DataValue<'a>) -> Result<bool> {
        match (self, other) {
            (DataValue::Number(a), DataValue::Number(b)) => Ok(a.as_f64() >= b.as_f64()),
            (DataValue::String(a), DataValue::String(b)) => Ok(*a >= *b),
            
            // Coerce to numbers for comparison
            (a, b) => {
                let a_num = a.coerce_to_number().ok_or_else(|| 
                    LogicError::type_mismatch("number", a.type_name().to_string()))?;
                let b_num = b.coerce_to_number().ok_or_else(|| 
                    LogicError::type_mismatch("number", b.type_name().to_string()))?;
                
                Ok(a_num.as_f64() >= b_num.as_f64())
            }
        }
    }
    
    fn less_than(&self, other: &DataValue<'a>) -> Result<bool> {
        match (self, other) {
            (DataValue::Number(a), DataValue::Number(b)) => Ok(a.as_f64() < b.as_f64()),
            (DataValue::String(a), DataValue::String(b)) => Ok(*a < *b),
            
            // Coerce to numbers for comparison
            (a, b) => {
                let a_num = a.coerce_to_number().ok_or_else(|| 
                    LogicError::type_mismatch("number", a.type_name().to_string()))?;
                let b_num = b.coerce_to_number().ok_or_else(|| 
                    LogicError::type_mismatch("number", b.type_name().to_string()))?;
                
                Ok(a_num.as_f64() < b_num.as_f64())
            }
        }
    }
    
    fn less_than_equal(&self, other: &DataValue<'a>) -> Result<bool> {
        match (self, other) {
            (DataValue::Number(a), DataValue::Number(b)) => Ok(a.as_f64() <= b.as_f64()),
            (DataValue::String(a), DataValue::String(b)) => Ok(*a <= *b),
            
            // Coerce to numbers for comparison
            (a, b) => {
                let a_num = a.coerce_to_number().ok_or_else(|| 
                    LogicError::type_mismatch("number", a.type_name().to_string()))?;
                let b_num = b.coerce_to_number().ok_or_else(|| 
                    LogicError::type_mismatch("number", b.type_name().to_string()))?;
                
                Ok(a_num.as_f64() <= b_num.as_f64())
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::arena::DataArena;
    
    #[test]
    fn test_coerce_to_bool() {
        let arena = DataArena::new();
        
        // Test null
        assert!(!DataValue::null().coerce_to_bool());
        
        // Test booleans
        assert!(DataValue::bool(true).coerce_to_bool());
        assert!(!DataValue::bool(false).coerce_to_bool());
        
        // Test numbers
        assert!(!DataValue::integer(0).coerce_to_bool());
        assert!(DataValue::integer(1).coerce_to_bool());
        assert!(DataValue::integer(-1).coerce_to_bool());
        
        // Test strings
        assert!(!DataValue::string(&arena, "").coerce_to_bool());
        assert!(DataValue::string(&arena, "hello").coerce_to_bool());
        
        // Test arrays
        assert!(!DataValue::array(&arena, &[]).coerce_to_bool());
        assert!(DataValue::array(&arena, &[DataValue::null()]).coerce_to_bool());
        
        // Test objects
        assert!(!DataValue::object(&arena, &[]).coerce_to_bool());
        assert!(DataValue::object(&arena, &[(arena.intern_str("key"), DataValue::null())]).coerce_to_bool());
    }
    
    #[test]
    fn test_coerce_to_number() {
        let arena = DataArena::new();
        
        // Test null
        assert_eq!(DataValue::null().coerce_to_number(), Some(NumberValue::Integer(0)));
        
        // Test booleans
        assert_eq!(DataValue::bool(true).coerce_to_number(), Some(NumberValue::Integer(1)));
        assert_eq!(DataValue::bool(false).coerce_to_number(), Some(NumberValue::Integer(0)));
        
        // Test numbers
        assert_eq!(DataValue::integer(42).coerce_to_number(), Some(NumberValue::Integer(42)));
        
        // Test strings
        assert_eq!(DataValue::string(&arena, "42").coerce_to_number(), Some(NumberValue::Integer(42)));
        assert_eq!(DataValue::string(&arena, "3.14").coerce_to_number().unwrap().as_f64(), 3.14);
        assert_eq!(DataValue::string(&arena, "not a number").coerce_to_number(), None);
        
        // Test arrays
        assert_eq!(DataValue::array(&arena, &[]).coerce_to_number(), Some(NumberValue::Integer(0)));
        assert_eq!(DataValue::array(&arena, &[DataValue::integer(42)]).coerce_to_number(), Some(NumberValue::Integer(42)));
        assert_eq!(DataValue::array(&arena, &[DataValue::integer(1), DataValue::integer(2)]).coerce_to_number(), None);
        
        // Test objects
        assert_eq!(DataValue::object(&arena, &[]).coerce_to_number(), None);
    }
    
    #[test]
    fn test_equals() {
        let arena = DataArena::new();
        
        // Same types
        assert!(DataValue::null().equals(&DataValue::null()));
        assert!(DataValue::bool(true).equals(&DataValue::bool(true)));
        assert!(!DataValue::bool(true).equals(&DataValue::bool(false)));
        assert!(DataValue::integer(42).equals(&DataValue::integer(42)));
        assert!(!DataValue::integer(42).equals(&DataValue::integer(43)));
        assert!(DataValue::string(&arena, "hello").equals(&DataValue::string(&arena, "hello")));
        assert!(!DataValue::string(&arena, "hello").equals(&DataValue::string(&arena, "world")));
        
        // Different types
        assert!(DataValue::null().equals(&DataValue::bool(false)));
        assert!(!DataValue::null().equals(&DataValue::bool(true)));
        assert!(DataValue::integer(42).equals(&DataValue::string(&arena, "42")));
        assert!(!DataValue::integer(42).equals(&DataValue::string(&arena, "43")));
    }
    
    #[test]
    fn test_comparison() {
        let arena = DataArena::new();
        
        // Greater than
        assert!(DataValue::integer(42).greater_than(&DataValue::integer(41)).unwrap());
        assert!(!DataValue::integer(42).greater_than(&DataValue::integer(42)).unwrap());
        assert!(!DataValue::integer(42).greater_than(&DataValue::integer(43)).unwrap());
        
        // Greater than or equal
        assert!(DataValue::integer(42).greater_than_equal(&DataValue::integer(41)).unwrap());
        assert!(DataValue::integer(42).greater_than_equal(&DataValue::integer(42)).unwrap());
        assert!(!DataValue::integer(42).greater_than_equal(&DataValue::integer(43)).unwrap());
        
        // Less than
        assert!(!DataValue::integer(42).less_than(&DataValue::integer(41)).unwrap());
        assert!(!DataValue::integer(42).less_than(&DataValue::integer(42)).unwrap());
        assert!(DataValue::integer(42).less_than(&DataValue::integer(43)).unwrap());
        
        // Less than or equal
        assert!(!DataValue::integer(42).less_than_equal(&DataValue::integer(41)).unwrap());
        assert!(DataValue::integer(42).less_than_equal(&DataValue::integer(42)).unwrap());
        assert!(DataValue::integer(42).less_than_equal(&DataValue::integer(43)).unwrap());
        
        // String comparison
        assert!(DataValue::string(&arena, "b").greater_than(&DataValue::string(&arena, "a")).unwrap());
        assert!(!DataValue::string(&arena, "a").greater_than(&DataValue::string(&arena, "b")).unwrap());
    }
} 