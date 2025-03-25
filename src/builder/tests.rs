#[cfg(test)]
mod tests {
    use crate::value::DataValue;
    use crate::DataLogic;

    #[test]
    fn test_basic_builder() {
        // Create JSONLogic instance with arena
        let logic = DataLogic::new();

        // Just parse a rule directly from JSON
        let ten_lt_twenty = logic.parse_logic(r#"{"<": [10, 20]}"#, None).unwrap();

        // Test the rule
        let data = logic.parse_data(r#"{}"#).unwrap();
        let result = logic.evaluate(&ten_lt_twenty, &data).unwrap();
        assert_eq!(result, &DataValue::Bool(true));
    }

    #[test]
    fn test_logic_builder() {
        // Create JSONLogic instance with arena
        let logic = DataLogic::new();

        // Create a JSON rule to verify against
        let json_str = r#"{"if": [{"==": [{"var": "status"}, "gold"]}, "Premium", "Basic"]}"#;

        // Parse it
        let rule_from_json = logic.parse_logic(json_str, None).unwrap();

        // Test the rule
        let gold_data = logic.parse_data(r#"{"status": "gold"}"#).unwrap();
        let normal_data = logic.parse_data(r#"{"status": "silver"}"#).unwrap();

        let result1 = logic.evaluate(&rule_from_json, &gold_data).unwrap();
        let result2 = logic.evaluate(&rule_from_json, &normal_data).unwrap();

        assert_eq!(result1, &DataValue::String("Premium"));
        assert_eq!(result2, &DataValue::String("Basic"));
    }
}
