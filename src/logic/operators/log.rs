//! Log operator implementation.
//!
//! This module provides the implementation of the log operator.

use crate::arena::DataArena;
use crate::logic::error::{LogicError, Result};
use crate::logic::evaluator::evaluate;
use crate::logic::token::Token;
use crate::value::DataValue;

/// Evaluates a log operation.
pub fn eval_log<'a>(
    args: &'a [&'a Token<'a>],
    data: &'a DataValue<'a>,
    arena: &'a DataArena,
) -> Result<&'a DataValue<'a>> {
    if args.is_empty() {
        return Err(LogicError::InvalidArgumentsError);
    }

    let value = evaluate(args[0], data, arena)?;
    
    // Debug logging - can be customized or controlled via feature flags
    // For now, just print to stderr
    eprintln!("LOG: {:?}", value);
    
    Ok(value)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::logic::parser::parse_str;
    use crate::value::FromJson;
    use crate::JsonLogic;
    use serde_json::json;

    #[test]
    fn test_evaluate_log() {
        // Create JSONLogic instance with arena
        let logic = JsonLogic::new();
        let arena = logic.arena();
        
        let data_json = json!({
            "value": "test message"
        });
        
        // Create a custom log operation with a variable operand
        // Since there's no direct builder method for custom operations,
        // we'll use the parse_str function
        let token = parse_str(r#"{"log": [{"var": "value"}]}"#, &arena).unwrap();
        
        // Evaluate directly with the token
        let data = DataValue::from_json(&data_json, &arena);
        let result = crate::logic::evaluator::evaluate(token, &data, &arena).unwrap();
        assert_eq!(result.as_str(), Some("test message"));
        
        // Alternative: with the builder pattern, create an array of args
        // and a custom operation
        // Note: This would work if RuleBuilder had a custom_op method
        /*
        let var_arg = builder.var("value").build();
        let args = vec![var_arg];
        let log_rule = Logic::custom_operator("log", args, arena);
        let result = logic.apply_logic(&log_rule, &data_json).unwrap();
        assert_eq!(result, json!("test message"));
        */
    }
} 