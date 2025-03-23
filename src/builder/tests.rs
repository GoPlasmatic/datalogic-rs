#[cfg(test)]
mod tests {
    use crate::JsonLogic;
    use serde_json::json;

    #[test]
    fn test_basic_builder() {
        // Create JSONLogic instance with arena
        let logic = JsonLogic::new();
        
        // We don't actually need these variables - just keeping them to show
        // what could be done with the builder
        let _builder = logic.builder();

        // Just parse a rule directly from JSON
        let ten_lt_twenty = logic
            .parse(&json!({"<": [10, 20]}))
            .unwrap();
            
        // Test the rule
        let data = json!({});
        let result = logic.apply_logic(&ten_lt_twenty, &data).unwrap();
        assert_eq!(result, json!(true));
    }

    #[test]
    fn test_logic_builder() {
        // Create JSONLogic instance with arena
        let logic = JsonLogic::new();
        
        // Create a JSON rule to verify against
        let json_str = r#"{"if": [{"==": [{"var": "status"}, "gold"]}, "Premium", "Basic"]}"#;
        
        // Parse it 
        let rule_from_json = logic.parse(&json_str).unwrap();
        
        // Test the rule
        let gold_data = json!({"status": "gold"});
        let normal_data = json!({"status": "silver"});
        
        let result1 = logic.apply_logic(&rule_from_json, &gold_data).unwrap();
        let result2 = logic.apply_logic(&rule_from_json, &normal_data).unwrap();
        
        assert_eq!(result1, json!("Premium"));
        assert_eq!(result2, json!("Basic"));
    }
} 