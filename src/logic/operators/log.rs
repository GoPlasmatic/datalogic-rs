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
        return Err(LogicError::OperatorError {
            operator: "log".to_string(),
            reason: "Expected at least 1 argument, got 0".to_string(),
        });
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