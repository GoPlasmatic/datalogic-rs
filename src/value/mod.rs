//! Value representation for efficient data processing.
//!
//! This module provides a memory-efficient value type that leverages arena allocation.
//! It replaces direct dependency on `serde_json::Value` with a custom implementation
//! optimized for rule evaluation.

mod access;
mod convert;
mod data_value;
mod number;

pub use access::{parse_path, PathSegment, ValueAccess};
pub use convert::{
    data_value_to_json, hash_map_to_data_value, json_to_data_value, FromJson, ToJson,
};
pub use data_value::DataValue;
pub use number::NumberValue;

use crate::arena::DataArena;

/// A trait for types that can be converted to a DataValue.
pub trait IntoDataValue<'a> {
    /// Converts the value to a DataValue, allocating in the given arena.
    fn into_data_value(self, arena: &'a DataArena) -> DataValue<'a>;
}

/// A trait for types that can be extracted from a DataValue.
pub trait FromDataValue<T> {
    /// Extracts a value of type T from a DataValue.
    fn from_data_value(value: &DataValue) -> Option<T>;
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::arena::DataArena;

    #[test]
    fn test_data_value_creation() {
        let arena = DataArena::new();

        // Create different types of values
        let null = DataValue::null();
        let boolean = DataValue::bool(true);
        let number = DataValue::integer(42);
        let string = DataValue::string(&arena, "hello");

        // Test basic properties
        assert!(null.is_null());
        assert!(boolean.is_bool());
        assert!(number.is_number());
        assert!(string.is_string());

        // Test value extraction
        assert_eq!(boolean.as_bool(), Some(true));
        assert_eq!(number.as_i64(), Some(42));
        assert_eq!(string.as_str(), Some("hello"));
    }

    #[test]
    fn test_array_and_object() {
        let arena = DataArena::new();

        // Create array using the array constructor method
        let array = DataValue::array(
            &arena,
            &[
                DataValue::integer(1),
                DataValue::integer(2),
                DataValue::integer(3),
            ],
        );

        assert!(array.is_array());
        assert_eq!(array.as_array().unwrap().len(), 3);

        // Create object using the object constructor method
        let object = DataValue::object(
            &arena,
            &[
                (arena.intern_str("a"), DataValue::integer(1)),
                (arena.intern_str("b"), DataValue::integer(2)),
            ],
        );

        assert!(object.is_object());
        assert_eq!(object.as_object().unwrap().len(), 2);
    }
}
