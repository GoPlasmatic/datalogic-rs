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
    args: &'a [Token<'a>],
    data: &'a DataValue<'a>,
    arena: &'a DataArena,
) -> Result<DataValue<'a>> {
    // Check that we have exactly 1 argument
    if args.len() != 1 {
        return Err(LogicError::OperatorError {
            operator: "log".to_string(),
            reason: format!("Expected 1 argument, got {}", args.len()),
        });
    }
    
    // Evaluate the argument
    let value = evaluate(&args[0], data, arena)?;
    
    // Log the value
    println!("LOG: {:?}", value);
    
    // Return the value
    Ok(value)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::logic::parser::parse_str;
    use crate::value::FromJson;
    use serde_json::json;

    #[test]
    fn test_evaluate_log() {
        let arena = DataArena::new();
        let data_json = json!({
            "value": "test message"
        });
        let data = DataValue::from_json(&data_json, &arena);
        
        // Parse and evaluate a log expression
        let token = parse_str(r#"{"log": [{"var": "value"}]}"#, &arena).unwrap();
        
        let result = crate::logic::evaluator::evaluate(token, &data, &arena).unwrap();
        assert_eq!(result.as_str(), Some("test message"));
    }
} 