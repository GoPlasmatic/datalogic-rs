//! Conversion utilities for DataValue.
//!
//! This module provides utilities for converting between DataValue and other formats,
//! such as JSON.

use super::data_value::DataValue;
use super::number::NumberValue;
use crate::arena::DataArena;
use serde_json::{Map as JsonMap, Number as JsonNumber, Value as JsonValue};
use std::collections::HashMap;

/// A trait for converting from JSON to DataValue.
pub trait FromJson<'a> {
    /// Converts a JSON value to a DataValue, allocating in the given arena.
    fn from_json(json: &JsonValue, arena: &'a DataArena) -> DataValue<'a>;
}

/// A trait for converting from DataValue to JSON.
pub trait ToJson {
    /// Converts a DataValue to a JSON value.
    fn to_json(&self) -> JsonValue;
}

impl<'a> FromJson<'a> for DataValue<'a> {
    fn from_json(json: &JsonValue, arena: &'a DataArena) -> DataValue<'a> {
        match json {
            JsonValue::Null => DataValue::null(),
            JsonValue::Bool(b) => DataValue::bool(*b),
            JsonValue::Number(n) => {
                if let Some(i) = n.as_i64() {
                    DataValue::integer(i)
                } else if let Some(f) = n.as_f64() {
                    DataValue::float(f)
                } else {
                    // This should never happen with valid JSON
                    DataValue::null()
                }
            }
            JsonValue::String(s) => DataValue::string(arena, s),
            JsonValue::Array(arr) => {
                // Pre-allocate space for the array elements
                let mut values = Vec::with_capacity(arr.len());

                // Convert each element in the array
                for item in arr.iter() {
                    values.push(DataValue::from_json(item, arena));
                }

                // Create the array DataValue
                DataValue::array(arena, &values)
            }
            JsonValue::Object(obj) => {
                // Check for special datetime/duration object patterns first
                if obj.len() == 1 {
                    if let Some(JsonValue::String(s)) = obj.get("datetime")
                        && let Ok(dt) = super::parse_datetime(s)
                    {
                        return DataValue::datetime(dt);
                    }

                    if let Some(JsonValue::String(s)) = obj.get("timestamp")
                        && let Ok(dur) = super::parse_duration(s)
                    {
                        return DataValue::duration(dur);
                    }
                }

                // Pre-allocate space for the object entries
                let mut entries = Vec::with_capacity(obj.len());

                // Convert each key-value pair in the object
                for (key, value) in obj.iter() {
                    let interned_key = arena.alloc_str(key);
                    entries.push((interned_key, DataValue::from_json(value, arena)));
                }

                // Create the object DataValue
                DataValue::object(arena, &entries)
            }
        }
    }
}

impl ToJson for DataValue<'_> {
    fn to_json(&self) -> JsonValue {
        match self {
            DataValue::Null => JsonValue::Null,
            DataValue::Bool(b) => JsonValue::Bool(*b),
            DataValue::Number(n) => {
                match n {
                    NumberValue::Integer(i) => {
                        // Create a JSON number directly from the integer to preserve its type
                        JsonValue::Number((*i).into())
                    }
                    NumberValue::Float(f) => {
                        if let Some(num) = JsonNumber::from_f64(*f) {
                            JsonValue::Number(num)
                        } else {
                            // Handle NaN, Infinity, etc.
                            JsonValue::Null
                        }
                    }
                }
            }
            DataValue::String(s) => JsonValue::String(s.to_string()),
            DataValue::Array(arr) => {
                let json_arr: Vec<JsonValue> = arr.iter().map(|item| item.to_json()).collect();
                JsonValue::Array(json_arr)
            }
            DataValue::Object(entries) => {
                let mut map = JsonMap::new();
                for (key, value) in entries.iter() {
                    map.insert((*key).to_string(), value.to_json());
                }
                JsonValue::Object(map)
            }
            DataValue::DateTime(dt) => {
                // Format with Z suffix for UTC, otherwise preserve timezone offset
                let formatted = if dt.offset().local_minus_utc() == 0 {
                    dt.format("%Y-%m-%dT%H:%M:%SZ").to_string()
                } else {
                    dt.to_rfc3339()
                };
                JsonValue::String(formatted)
            }
            DataValue::Duration(d) => {
                // Format the duration as a simplified string representation
                let total_seconds = d.num_seconds();
                let days = total_seconds / 86400;
                let hours = (total_seconds % 86400) / 3600;
                let minutes = (total_seconds % 3600) / 60;
                let seconds = total_seconds % 60;

                if days > 0 {
                    JsonValue::String(format!("{days}d:{hours}h:{minutes}m:{seconds}s"))
                } else if hours > 0 {
                    JsonValue::String(format!("{hours}h:{minutes}m:{seconds}s"))
                } else if minutes > 0 {
                    JsonValue::String(format!("{minutes}m:{seconds}s"))
                } else {
                    JsonValue::String(format!("{seconds}s"))
                }
            }
        }
    }
}

/// Converts a JSON value to a DataValue.
pub fn json_to_data_value<'a>(json: &JsonValue, arena: &'a DataArena) -> DataValue<'a> {
    DataValue::from_json(json, arena)
}

/// Converts a DataValue to a JSON value.
pub fn data_value_to_json(value: &DataValue<'_>) -> JsonValue {
    value.to_json()
}

/// Converts a HashMap to a DataValue object.
pub fn hash_map_to_data_value<'a, V>(
    map: &HashMap<String, V>,
    arena: &'a DataArena,
    value_converter: impl Fn(&V, &'a DataArena) -> DataValue<'a>,
) -> DataValue<'a> {
    let entries: Vec<(&'a str, DataValue<'a>)> = map
        .iter()
        .map(|(key, value)| {
            let interned_key = arena.alloc_str(key);
            let data_value = value_converter(value, arena);
            (interned_key, data_value)
        })
        .collect();

    // Create the object DataValue
    DataValue::object(arena, &entries)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn test_json_conversion() {
        let arena = DataArena::new();

        // Create a complex JSON value
        let json = json!({
            "null": null,
            "bool": true,
            "integer": 42,
            "float": 3.5,
            "string": "hello",
            "array": [1, 2, 3],
            "object": {
                "a": 1,
                "b": "two"
            }
        });

        // Convert JSON to DataValue
        let data_value = DataValue::from_json(&json, &arena);

        // Convert back to JSON
        let json2 = data_value.to_json();

        // Verify the round-trip conversion
        assert_eq!(json, json2);
    }

    #[test]
    fn test_hash_map_conversion() {
        let arena = DataArena::new();

        // Create a HashMap
        let mut map = HashMap::new();
        map.insert("a".to_string(), 1);
        map.insert("b".to_string(), 2);
        map.insert("c".to_string(), 3);

        // Convert HashMap to DataValue
        let data_value = hash_map_to_data_value(&map, &arena, |v, _| DataValue::integer(*v));

        // Verify the conversion
        if let DataValue::Object(entries) = data_value {
            assert_eq!(entries.len(), 3);

            // Check each entry
            let mut found_a = false;
            let mut found_b = false;
            let mut found_c = false;

            for (key, value) in entries.iter() {
                let v = value.as_i64().unwrap();

                match *key {
                    "a" => {
                        assert_eq!(v, 1);
                        found_a = true;
                    }
                    "b" => {
                        assert_eq!(v, 2);
                        found_b = true;
                    }
                    "c" => {
                        assert_eq!(v, 3);
                        found_c = true;
                    }
                    _ => panic!("Unexpected key: {key}"),
                }
            }

            assert!(
                found_a && found_b && found_c,
                "Not all expected keys were found"
            );
        } else {
            panic!("Expected DataValue::Object");
        }
    }

    #[test]
    fn test_json_to_data_value() {
        let arena = DataArena::new();

        // Create a JSON value
        let json = json!({
            "name": "John",
            "age": 30,
            "is_active": true
        });

        // Convert JSON to DataValue using the helper function
        let data_value = json_to_data_value(&json, &arena);

        // Verify the conversion
        assert!(data_value.is_object());
        let obj = data_value.as_object().unwrap();

        // Find and verify each field
        let mut found_name = false;
        let mut found_age = false;
        let mut found_is_active = false;

        for (key, value) in obj.iter() {
            match *key {
                "name" => {
                    assert_eq!(value.as_str(), Some("John"));
                    found_name = true;
                }
                "age" => {
                    assert_eq!(value.as_i64(), Some(30));
                    found_age = true;
                }
                "is_active" => {
                    assert_eq!(value.as_bool(), Some(true));
                    found_is_active = true;
                }
                _ => panic!("Unexpected key: {key}"),
            }
        }

        assert!(
            found_name && found_age && found_is_active,
            "Not all expected keys were found"
        );
    }

    #[test]
    fn test_data_value_to_json() {
        let arena = DataArena::new();

        // Create a DataValue
        let data_value = DataValue::object(
            &arena,
            &[
                (arena.alloc_str("name"), DataValue::string(&arena, "Alice")),
                (
                    arena.alloc_str("scores"),
                    DataValue::array(
                        &arena,
                        &[
                            DataValue::integer(95),
                            DataValue::integer(87),
                            DataValue::integer(92),
                        ],
                    ),
                ),
            ],
        );

        // Convert DataValue to JSON using the helper function
        let json = data_value_to_json(&data_value);

        // Verify the conversion
        if let JsonValue::Object(map) = json {
            assert_eq!(map.len(), 2);

            // Check name field
            if let Some(JsonValue::String(name)) = map.get("name") {
                assert_eq!(name, "Alice");
            } else {
                panic!("Expected 'name' to be a string");
            }

            // Check scores field
            if let Some(JsonValue::Array(scores)) = map.get("scores") {
                assert_eq!(scores.len(), 3);
                assert_eq!(scores[0], JsonValue::Number(95.into()));
                assert_eq!(scores[1], JsonValue::Number(87.into()));
                assert_eq!(scores[2], JsonValue::Number(92.into()));
            } else {
                panic!("Expected 'scores' to be an array");
            }
        } else {
            panic!("Expected a JSON object");
        }
    }

    #[test]
    fn test_hash_map_to_data_value_with_complex_values() {
        let arena = DataArena::new();

        // Create a HashMap with complex values (nested structures)
        let mut map = HashMap::new();
        map.insert("user1".to_string(), ("Alice", 25));
        map.insert("user2".to_string(), ("Bob", 30));

        // Convert HashMap to DataValue with a custom converter
        let data_value = hash_map_to_data_value(&map, &arena, |&(name, age), arena| {
            DataValue::object(
                arena,
                &[
                    (arena.alloc_str("name"), DataValue::string(arena, name)),
                    (arena.alloc_str("age"), DataValue::integer(age)),
                ],
            )
        });

        // Verify the conversion
        if let DataValue::Object(entries) = data_value {
            assert_eq!(entries.len(), 2);

            // Check each user
            for (key, value) in entries.iter() {
                match *key {
                    "user1" => {
                        if let DataValue::Object(user_entries) = value {
                            let mut found_name = false;
                            let mut found_age = false;

                            for (user_key, user_value) in user_entries.iter() {
                                match *user_key {
                                    "name" => {
                                        assert_eq!(user_value.as_str(), Some("Alice"));
                                        found_name = true;
                                    }
                                    "age" => {
                                        assert_eq!(user_value.as_i64(), Some(25));
                                        found_age = true;
                                    }
                                    _ => panic!("Unexpected user key: {user_key}"),
                                }
                            }

                            assert!(
                                found_name && found_age,
                                "Not all expected user fields were found"
                            );
                        } else {
                            panic!("Expected user1 to be an object");
                        }
                    }
                    "user2" => {
                        if let DataValue::Object(user_entries) = value {
                            let mut found_name = false;
                            let mut found_age = false;

                            for (user_key, user_value) in user_entries.iter() {
                                match *user_key {
                                    "name" => {
                                        assert_eq!(user_value.as_str(), Some("Bob"));
                                        found_name = true;
                                    }
                                    "age" => {
                                        assert_eq!(user_value.as_i64(), Some(30));
                                        found_age = true;
                                    }
                                    _ => panic!("Unexpected user key: {user_key}"),
                                }
                            }

                            assert!(
                                found_name && found_age,
                                "Not all expected user fields were found"
                            );
                        } else {
                            panic!("Expected user2 to be an object");
                        }
                    }
                    _ => panic!("Unexpected key: {key}"),
                }
            }
        } else {
            panic!("Expected DataValue::Object");
        }
    }
}
