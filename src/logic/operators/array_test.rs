//! Tests for array operators.

#[cfg(test)]
mod tests {
    use crate::arena::DataArena;
    use crate::value::DataValue;
    use crate::logic::parser::parse_str;
    use crate::logic::evaluator::evaluate;
    use crate::value::FromJson;
    use serde_json::json;

    #[test]
    fn test_map_operator() {
        let arena = DataArena::new();
        
        // Test case 1: Map integers to double their value
        let data_json = json!({
            "integers": [1, 2, 3]
        });
        let data = DataValue::from_json(&data_json, &arena);
        
        let rule_str = r#"{"map": [{"var": "integers"}, {"*": [{"var": ""}, 2]}]}"#;
        let token = parse_str(rule_str, &arena).unwrap();
        let result = evaluate(token, &data, &arena).unwrap();
        
        // Check that the result is an array with the expected values
        assert!(result.is_array());
        let result_array = result.as_array().unwrap();
        assert_eq!(result_array.len(), 3);
        assert_eq!(result_array[0].as_i64(), Some(2));
        assert_eq!(result_array[1].as_i64(), Some(4));
        assert_eq!(result_array[2].as_i64(), Some(6));
        
        // Test case 2: Map with null data should return empty array
        let null_data = DataValue::null();
        let result = evaluate(token, &null_data, &arena).unwrap();
        assert!(result.is_array());
        assert_eq!(result.as_array().unwrap().len(), 0);
        
        // Test case 3: Map with object array
        let desserts_json = json!({
            "desserts": [
                {"name": "apple", "qty": 1},
                {"name": "brownie", "qty": 2},
                {"name": "cupcake", "qty": 3}
            ]
        });
        let desserts_data = DataValue::from_json(&desserts_json, &arena);
        
        let qty_rule_str = r#"{"map": [{"var": "desserts"}, {"var": "qty"}]}"#;
        let qty_token = parse_str(qty_rule_str, &arena).unwrap();
        let qty_result = evaluate(qty_token, &desserts_data, &arena).unwrap();
        
        // Check that the result is an array with the expected values
        assert!(qty_result.is_array());
        let qty_array = qty_result.as_array().unwrap();
        assert_eq!(qty_array.len(), 3);
        assert_eq!(qty_array[0].as_i64(), Some(1));
        assert_eq!(qty_array[1].as_i64(), Some(2));
        assert_eq!(qty_array[2].as_i64(), Some(3));
    }
    
    #[test]
    fn test_filter_operator() {
        let arena = DataArena::new();
        
        // Test case 1: Filter integers greater than or equal to 2
        let data_json = json!({
            "integers": [1, 2, 3]
        });
        let data = DataValue::from_json(&data_json, &arena);
        
        let rule_str = r#"{"filter": [{"var": "integers"}, {">=": [{"var": ""}, 2]}]}"#;
        let token = parse_str(rule_str, &arena).unwrap();
        let result = evaluate(token, &data, &arena).unwrap();
        
        // Check that the result is an array with the expected values
        assert!(result.is_array());
        let result_array = result.as_array().unwrap();
        assert_eq!(result_array.len(), 2);
        assert_eq!(result_array[0].as_i64(), Some(2));
        assert_eq!(result_array[1].as_i64(), Some(3));
        
        // Test case 2: Filter with constant true (should return all elements)
        let true_rule_str = r#"{"filter": [{"var": "integers"}, true]}"#;
        let true_token = parse_str(true_rule_str, &arena).unwrap();
        let true_result = evaluate(true_token, &data, &arena).unwrap();
        
        assert!(true_result.is_array());
        let true_array = true_result.as_array().unwrap();
        assert_eq!(true_array.len(), 3);
        
        // Test case 3: Filter with constant false (should return empty array)
        let false_rule_str = r#"{"filter": [{"var": "integers"}, false]}"#;
        let false_token = parse_str(false_rule_str, &arena).unwrap();
        let false_result = evaluate(false_token, &data, &arena).unwrap();
        
        assert!(false_result.is_array());
        assert_eq!(false_result.as_array().unwrap().len(), 0);
        
        // Test case 4: Filter odd numbers (using modulo)
        let odd_rule_str = r#"{"filter": [{"var": "integers"}, {"%": [{"var": ""}, 2]}]}"#;
        let odd_token = parse_str(odd_rule_str, &arena).unwrap();
        let odd_result = evaluate(odd_token, &data, &arena).unwrap();
        
        assert!(odd_result.is_array());
        let odd_array = odd_result.as_array().unwrap();
        assert_eq!(odd_array.len(), 2);
        assert_eq!(odd_array[0].as_i64(), Some(1));
        assert_eq!(odd_array[1].as_i64(), Some(3));
    }
} 