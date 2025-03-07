//! Variable operator implementation.
//!
//! This module provides the implementation of the variable operator.

use crate::arena::DataArena;
use crate::logic::error::Result;
use crate::logic::evaluator::evaluate;
use crate::logic::token::Token;
use crate::value::DataValue;

/// Evaluates a variable reference.
pub fn evaluate_variable<'a>(
    path: &str,
    default: &Option<&'a Token<'a>>,
    data: &'a DataValue<'a>,
    arena: &'a DataArena,
) -> Result<DataValue<'a>> {
    // Handle empty path as a reference to the data itself
    if path.is_empty() {
        return Ok(data.clone());
    }
    
    // Fast path for direct property access (no dots)
    if !path.contains('.') {
        // Special case for numeric indices - direct array access
        if let Ok(index) = path.parse::<usize>() {
            if let DataValue::Array(items) = data {
                if index < items.len() {
                    return Ok(items[index].clone());
                }
            }
            
            // Not found, use default
            if let Some(default_token) = default {
                return evaluate(default_token, data, arena);
            }
            
            return Ok(DataValue::null());
        }

        if let DataValue::Object(obj) = data {
            for (k, v) in *obj {
                if *k == path {
                    return Ok(v.clone());
                }
            }
        }
        
        // Not found, use default
        if let Some(default_token) = default {
            return evaluate(default_token, data, arena);
        }
        
        return Ok(DataValue::null());
    }

    // Navigate through the data without collecting into a Vec
    let mut current = data;
    let path_iter = path.split('.');
    
    for component in path_iter {
        match current {
            DataValue::Object(entries) => {
                // Look for the component in the object
                let mut found = false;
                for (key, value) in entries.iter() {
                    if *key == component {
                        current = value;
                        found = true;
                        break;
                    }
                }
                
                // If the component wasn't found, use the default or return null
                if !found {
                    if let Some(default_token) = default {
                        return evaluate(default_token, data, arena);
                    } else {
                        // Return null for missing properties (JSONLogic behavior)
                        return Ok(DataValue::null());
                    }
                }
            },
            DataValue::Array(items) => {
                // Try to parse the component as an index
                if let Ok(index) = component.parse::<usize>() {
                    if index < items.len() {
                        current = &items[index];
                    } else {
                        // Index out of bounds, use the default or return null
                        if let Some(default_token) = default {
                            return evaluate(default_token, data, arena);
                        } else {
                            // Return null for out-of-bounds indices (JSONLogic behavior)
                            return Ok(DataValue::null());
                        }
                    }
                } else {
                    // Not a valid index, use the default or return null
                    if let Some(default_token) = default {
                        return evaluate(default_token, data, arena);
                    } else {
                        // Return null for invalid indices (JSONLogic behavior)
                        return Ok(DataValue::null());
                    }
                }
            },
            _ => {
                // Not an object or array, use the default or return null
                if let Some(default_token) = default {
                    return evaluate(default_token, data, arena);
                } else {
                    // Return null for non-object/non-array access (JSONLogic behavior)
                    return Ok(DataValue::null());
                }
            },
        }
    }
    
    // Return the final value
    Ok(current.clone())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::logic::parser::parse_str;
    use crate::value::FromJson;
    use serde_json::json;

    #[test]
    fn test_evaluate_variable() {
        let arena = DataArena::new();
        let data_json = json!({
            "user": {
                "name": "Alice",
                "age": 30
            }
        });
        let data = DataValue::from_json(&data_json, &arena);
        
        // Parse and evaluate a variable
        let token = parse_str(r#"{"var": "user.name"}"#, &arena).unwrap();
        let result = crate::logic::evaluator::evaluate(token, &data, &arena).unwrap();
        assert_eq!(result.as_str(), Some("Alice"));
        
        // Test with default value (not used)
        let token = parse_str(r#"{"var": ["user.name", "Bob"]}"#, &arena).unwrap();
        let result = crate::logic::evaluator::evaluate(token, &data, &arena).unwrap();
        assert_eq!(result.as_str(), Some("Alice"));
        
        // Test with default value (used)
        let token = parse_str(r#"{"var": ["user.email", "bob@example.com"]}"#, &arena).unwrap();
        let result = crate::logic::evaluator::evaluate(token, &data, &arena).unwrap();
        assert_eq!(result.as_str(), Some("bob@example.com"));
    }
} 