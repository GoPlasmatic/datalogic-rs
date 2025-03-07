//! Core DataValue implementation.
//!
//! This module provides the DataValue enum, which is a memory-efficient
//! representation of data values that leverages arena allocation.

use std::fmt;
use std::cmp::Ordering;
use crate::arena::DataArena;
use super::number::NumberValue;

/// A memory-efficient value type that leverages arena allocation.
///
/// This replaces the direct dependency on `serde_json::Value` with a custom
/// implementation optimized for rule evaluation.
#[derive(Debug, Clone, PartialEq)]
pub enum DataValue<'a> {
    /// Represents a null value
    Null,
    
    /// Represents a boolean value
    Bool(bool),
    
    /// Represents a numeric value (integer or floating point)
    Number(NumberValue),
    
    /// Represents a string value (arena-allocated)
    String(&'a str),
    
    /// Represents an array of values (arena-allocated)
    Array(&'a [DataValue<'a>]),
    
    /// Represents an object with key-value pairs (arena-allocated)
    Object(&'a [(&'a str, DataValue<'a>)]),
}

impl<'a> DataValue<'a> {
    /// Creates a null value.
    pub fn null() -> Self {
        DataValue::Null
    }
    
    /// Creates a boolean value.
    pub fn bool(value: bool) -> Self {
        DataValue::Bool(value)
    }
    
    /// Creates an integer value.
    pub fn integer(value: i64) -> Self {
        DataValue::Number(NumberValue::Integer(value))
    }
    
    /// Creates a floating-point value.
    pub fn float(value: f64) -> Self {
        DataValue::Number(NumberValue::from_f64(value))
    }
    
    /// Creates a string value.
    pub fn string(arena: &'a DataArena, value: &str) -> Self {
        DataValue::String(arena.alloc_str(value))
    }
    
    /// Creates an array value.
    pub fn array(arena: &'a DataArena, values: &[DataValue<'a>]) -> Self {
        DataValue::Array(arena.alloc_slice_clone(values))
    }
    
    /// Creates an object value.
    pub fn object(arena: &'a DataArena, entries: &[(&'a str, DataValue<'a>)]) -> Self {
        DataValue::Object(arena.alloc_slice_clone(entries))
    }
    
    /// Returns true if the value is null.
    pub fn is_null(&self) -> bool {
        matches!(self, DataValue::Null)
    }
    
    /// Returns true if the value is a boolean.
    pub fn is_bool(&self) -> bool {
        matches!(self, DataValue::Bool(_))
    }
    
    /// Returns true if the value is a number.
    pub fn is_number(&self) -> bool {
        matches!(self, DataValue::Number(_))
    }
    
    /// Returns true if the value is a string.
    pub fn is_string(&self) -> bool {
        matches!(self, DataValue::String(_))
    }
    
    /// Returns true if the value is an array.
    pub fn is_array(&self) -> bool {
        matches!(self, DataValue::Array(_))
    }
    
    /// Returns true if the value is an object.
    pub fn is_object(&self) -> bool {
        matches!(self, DataValue::Object(_))
    }
    
    /// Returns the value as a boolean, if it is one.
    pub fn as_bool(&self) -> Option<bool> {
        match self {
            DataValue::Bool(b) => Some(*b),
            _ => None,
        }
    }
    
    /// Returns the value as an i64, if it is a number that can be represented as an i64.
    pub fn as_i64(&self) -> Option<i64> {
        match self {
            DataValue::Number(n) => n.as_i64(),
            _ => None,
        }
    }
    
    /// Returns the value as an f64, if it is a number.
    pub fn as_f64(&self) -> Option<f64> {
        match self {
            DataValue::Number(n) => Some(n.as_f64()),
            _ => None,
        }
    }
    
    /// Returns the value as a string slice, if it is a string.
    pub fn as_str(&self) -> Option<&str> {
        match self {
            DataValue::String(s) => Some(s),
            _ => None,
        }
    }
    
    /// Returns the value as an array slice, if it is an array.
    pub fn as_array(&self) -> Option<&[DataValue<'a>]> {
        match self {
            DataValue::Array(a) => Some(a),
            _ => None,
        }
    }
    
    /// Returns the value as an object slice, if it is an object.
    pub fn as_object(&self) -> Option<&[(&'a str, DataValue<'a>)]> {
        match self {
            DataValue::Object(o) => Some(o),
            _ => None,
        }
    }
    
    /// Coerces the value to a boolean according to JSONLogic rules.
    pub fn coerce_to_bool(&self) -> bool {
        match self {
            DataValue::Bool(b) => *b,
            DataValue::Null => false,
            DataValue::Number(n) => match n {
                NumberValue::Integer(i) => *i != 0,
                NumberValue::Float(f) => *f != 0.0 && !f.is_nan(),
            },
            DataValue::String(s) => !s.is_empty(),
            DataValue::Array(a) => !a.is_empty(),
            DataValue::Object(o) => !o.is_empty(),
        }
    }
    
    /// Coerces the value to a number according to JSONLogic rules.
    pub fn coerce_to_number(&self) -> Option<NumberValue> {
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
    
    /// Coerces the value to a string according to JSONLogic rules.
    pub fn coerce_to_string(&self, arena: &'a DataArena) -> DataValue<'a> {
        match self {
            DataValue::Null => DataValue::String(arena.alloc_str("null")),
            DataValue::Bool(b) => DataValue::String(arena.alloc_str(if *b { "true" } else { "false" })),
            DataValue::Number(n) => DataValue::String(arena.alloc_str(&n.to_string())),
            DataValue::String(s) => DataValue::String(*s),
            DataValue::Array(a) => {
                let mut result = String::new();
                for (i, v) in a.iter().enumerate() {
                    if i > 0 {
                        result.push(',');
                    }
                    if let DataValue::String(s) = v.coerce_to_string(arena) {
                        result.push_str(s);
                    }
                }
                DataValue::String(arena.alloc_str(&result))
            },
            DataValue::Object(_) => DataValue::String(arena.alloc_str("[object Object]")),
        }
    }
    
    /// Gets a value from an object by key.
    pub fn get(&self, key: &str) -> Option<&DataValue<'a>> {
        match self {
            DataValue::Object(entries) => entries.binary_search_by_key(&key, |&(k, _)| k)
                .ok()
                .map(|idx| &entries[idx].1),
            _ => None,
        }
    }
    
    /// Gets a value from an array by index.
    pub fn get_index(&self, index: usize) -> Option<&DataValue<'a>> {
        match self {
            DataValue::Array(elements) => elements.get(index),
            _ => None,
        }
    }
    
    /// Returns a string representation of the type of this value.
    pub fn type_name(&self) -> &'static str {
        match self {
            DataValue::Null => "null",
            DataValue::Bool(_) => "boolean",
            DataValue::Number(_) => "number",
            DataValue::String(_) => "string",
            DataValue::Array(_) => "array",
            DataValue::Object(_) => "object",
        }
    }
    
    /// Checks if this value equals another value, with type coercion.
    pub fn equals(&self, other: &DataValue<'a>) -> bool {
        match (self, other) {
            // Same types use direct comparison
            (DataValue::Null, DataValue::Null) => true,
            (DataValue::Bool(a), DataValue::Bool(b)) => a == b,
            (DataValue::Number(a), DataValue::Number(b)) => a == b,
            (DataValue::String(a), DataValue::String(b)) => a == b,
            
            // Different types with coercion
            (DataValue::Null, DataValue::Bool(b)) => !b,
            (DataValue::Bool(a), DataValue::Null) => !a,
            
            (DataValue::Number(a), DataValue::String(b)) => {
                if let Ok(b_num) = b.parse::<f64>() {
                    return a.as_f64() == b_num;
                }
                false
            },
            (DataValue::String(a), DataValue::Number(b)) => {
                if let Ok(a_num) = a.parse::<f64>() {
                    return a_num == b.as_f64();
                }
                false
            },
            
            // Arrays and objects are compared by reference
            (DataValue::Array(a), DataValue::Array(b)) => {
                if a.len() != b.len() {
                    return false;
                }
                a.iter().zip(b.iter()).all(|(a_item, b_item)| a_item.equals(b_item))
            },
            (DataValue::Object(a), DataValue::Object(b)) => {
                if a.len() != b.len() {
                    return false;
                }
                
                // Check that all keys in a exist in b with equal values
                for (a_key, a_value) in *a {
                    let mut found = false;
                    for (b_key, b_value) in *b {
                        if a_key == b_key {
                            if !a_value.equals(b_value) {
                                return false;
                            }
                            found = true;
                            break;
                        }
                    }
                    if !found {
                        return false;
                    }
                }
                true
            },
            
            // Other combinations are not equal
            _ => false,
        }
    }
    
    /// Checks if this value strictly equals another value, without type coercion.
    pub fn strict_equals(&self, other: &DataValue<'a>) -> bool {
        match (self, other) {
            (DataValue::Null, DataValue::Null) => true,
            (DataValue::Bool(a), DataValue::Bool(b)) => a == b,
            (DataValue::Number(a), DataValue::Number(b)) => a == b,
            (DataValue::String(a), DataValue::String(b)) => a == b,
            (DataValue::Array(a), DataValue::Array(b)) => {
                if a.len() != b.len() {
                    return false;
                }
                a.iter().zip(b.iter()).all(|(a_item, b_item)| a_item.strict_equals(b_item))
            },
            (DataValue::Object(a), DataValue::Object(b)) => {
                if a.len() != b.len() {
                    return false;
                }
                
                // Check that all keys in a exist in b with strictly equal values
                for (a_key, a_value) in *a {
                    let mut found = false;
                    for (b_key, b_value) in *b {
                        if a_key == b_key {
                            if !a_value.strict_equals(b_value) {
                                return false;
                            }
                            found = true;
                            break;
                        }
                    }
                    if !found {
                        return false;
                    }
                }
                true
            },
            _ => false, // Different types are never strictly equal
        }
    }
}

impl<'a> PartialOrd for DataValue<'a> {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        match (self, other) {
            (DataValue::Null, DataValue::Null) => Some(Ordering::Equal),
            (DataValue::Bool(a), DataValue::Bool(b)) => a.partial_cmp(b),
            (DataValue::Number(a), DataValue::Number(b)) => a.partial_cmp(b),
            (DataValue::String(a), DataValue::String(b)) => a.partial_cmp(b),
            (DataValue::Array(a), DataValue::Array(b)) => {
                // Compare arrays lexicographically
                let min_len = a.len().min(b.len());
                for i in 0..min_len {
                    match a[i].partial_cmp(&b[i]) {
                        Some(Ordering::Equal) => continue,
                        other => return other,
                    }
                }
                a.len().partial_cmp(&b.len())
            },
            // Mixed types: convert to common type for comparison
            (DataValue::Number(a), DataValue::String(b)) => {
                if let Ok(b_num) = b.parse::<f64>() {
                    a.as_f64().partial_cmp(&b_num)
                } else {
                    None
                }
            },
            (DataValue::String(a), DataValue::Number(b)) => {
                if let Ok(a_num) = a.parse::<f64>() {
                    a_num.partial_cmp(&b.as_f64())
                } else {
                    None
                }
            },
            // Other combinations are not comparable
            _ => None,
        }
    }
}

impl<'a> fmt::Display for DataValue<'a> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            DataValue::Null => write!(f, "null"),
            DataValue::Bool(b) => write!(f, "{}", b),
            DataValue::Number(n) => write!(f, "{}", n),
            DataValue::String(s) => write!(f, "\"{}\"", s.replace('"', "\\\"")),
            DataValue::Array(a) => {
                write!(f, "[")?;
                for (i, v) in a.iter().enumerate() {
                    if i > 0 {
                        write!(f, ", ")?;
                    }
                    write!(f, "{}", v)?;
                }
                write!(f, "]")
            },
            DataValue::Object(o) => {
                write!(f, "{{")?;
                for (i, (k, v)) in o.iter().enumerate() {
                    if i > 0 {
                        write!(f, ", ")?;
                    }
                    write!(f, "\"{}\": {}", k, v)?;
                }
                write!(f, "}}")
            },
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::arena::DataArena;

    #[test]
    fn test_data_value_creation() {
        let arena = DataArena::new();
        
        let null = DataValue::null();
        let boolean = DataValue::bool(true);
        let integer = DataValue::integer(42);
        let float = DataValue::float(3.14);
        let string = DataValue::string(&arena, "hello");
        
        assert!(null.is_null());
        assert!(boolean.is_bool());
        assert!(integer.is_number());
        assert!(float.is_number());
        assert!(string.is_string());
        
        assert_eq!(boolean.as_bool(), Some(true));
        assert_eq!(integer.as_i64(), Some(42));
        assert_eq!(float.as_f64(), Some(3.14));
        assert_eq!(string.as_str(), Some("hello"));
    }

    #[test]
    fn test_array_and_object() {
        let arena = DataArena::new();
        
        // Create array
        let array = DataValue::array(&arena, &[
            DataValue::integer(1),
            DataValue::integer(2),
            DataValue::integer(3),
        ]);
        
        assert!(array.is_array());
        assert_eq!(array.as_array().unwrap().len(), 3);
        assert_eq!(array.get_index(1).unwrap().as_i64(), Some(2));
        
        // Create object
        let key1 = arena.intern_str("a");
        let key2 = arena.intern_str("b");
        
        let object = DataValue::object(&arena, &[
            (key1, DataValue::integer(1)),
            (key2, DataValue::integer(2)),
        ]);
        
        assert!(object.is_object());
        assert_eq!(object.as_object().unwrap().len(), 2);
        assert_eq!(object.get("a").unwrap().as_i64(), Some(1));
    }

    #[test]
    fn test_coercion() {
        let arena = DataArena::new();
        
        // Boolean coercion
        assert_eq!(DataValue::null().coerce_to_bool(), false);
        assert_eq!(DataValue::bool(true).coerce_to_bool(), true);
        assert_eq!(DataValue::integer(0).coerce_to_bool(), false);
        assert_eq!(DataValue::integer(1).coerce_to_bool(), true);
        assert_eq!(DataValue::string(&arena, "").coerce_to_bool(), false);
        assert_eq!(DataValue::string(&arena, "hello").coerce_to_bool(), true);
        
        // Number coercion
        assert_eq!(DataValue::null().coerce_to_number(), Some(NumberValue::Integer(0)));
        assert_eq!(DataValue::bool(true).coerce_to_number(), Some(NumberValue::Integer(1)));
        assert_eq!(DataValue::string(&arena, "42").coerce_to_number(), Some(NumberValue::Integer(42)));
        assert_eq!(DataValue::string(&arena, "3.14").coerce_to_number(), Some(NumberValue::Float(3.14)));
        
        // String coercion
        assert_eq!(DataValue::null().coerce_to_string(&arena).as_str(), Some("null"));
        assert_eq!(DataValue::bool(true).coerce_to_string(&arena).as_str(), Some("true"));
        assert_eq!(DataValue::integer(42).coerce_to_string(&arena).as_str(), Some("42"));
    }

    #[test]
    fn test_comparison() {
        let arena = DataArena::new();
        
        // Same types
        assert!(DataValue::null() == DataValue::null());
        assert!(DataValue::bool(true) > DataValue::bool(false));
        assert!(DataValue::integer(5) > DataValue::integer(3));
        assert!(DataValue::float(3.14) > DataValue::float(2.71));
        assert!(DataValue::string(&arena, "hello") == DataValue::string(&arena, "hello"));
        assert!(DataValue::string(&arena, "world") > DataValue::string(&arena, "hello"));
        
        // Mixed types
        assert!(DataValue::integer(42) == DataValue::float(42.0));
        
        // Array comparison
        let array1 = DataValue::array(&arena, &[DataValue::integer(1), DataValue::integer(2)]);
        let array2 = DataValue::array(&arena, &[DataValue::integer(1), DataValue::integer(3)]);
        assert!(array1 < array2);
    }
}
