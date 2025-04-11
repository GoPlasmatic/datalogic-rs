//! Missing operators for logic expressions.
//!
//! This module provides implementations for missing operators
//! such as missing and missing_some.

use crate::arena::DataArena;
use crate::logic::error::{LogicError, Result};
use crate::logic::evaluator::evaluate;
use crate::logic::operators::variable;
use crate::logic::token::Token;
use crate::value::DataValue;

/// Checks if a variable with the given name exists and is not null
fn variable_exists<'a>(name: &'a str, arena: &'a DataArena) -> bool {
    let none_ref: Option<&Token> = None;
    if let Ok(var_value) = variable::evaluate_variable(name, &none_ref, arena) {
        return var_value != arena.null_value();
    }
    false
}

/// Evaluates a missing operation.
/// Checks whether the specified variables are missing from the data.
pub fn eval_missing<'a>(
    args: &'a [&'a Token<'a>],
    arena: &'a DataArena,
) -> Result<&'a DataValue<'a>> {
    if args.is_empty() {
        return Ok(arena.empty_array_value());
    }

    let mut missing = arena.get_data_value_vec();

    for arg in args {
        let value = evaluate(arg, arena)?;

        match value {
            DataValue::String(name) => {
                if !variable_exists(name, arena) {
                    missing.push(DataValue::String(name));
                }
            }
            DataValue::Array(names) => {
                // Process each variable name in the array
                for name_value in *names {
                    if let DataValue::String(name) = name_value {
                        if !variable_exists(name, arena) {
                            missing.push(DataValue::String(name));
                        }
                    }
                    // Ignore non-string names
                }
            }
            // Ignore non-string, non-array values
            _ => {}
        }
    }

    let result = DataValue::Array(arena.bump_vec_into_slice(missing));
    Ok(arena.alloc(result))
}

/// Evaluates a missing_some operation.
/// Checks whether at least the specified number of variables are present in the data.
pub fn eval_missing_some<'a>(
    args: &'a [&'a Token<'a>],
    arena: &'a DataArena,
) -> Result<&'a DataValue<'a>> {
    if args.len() != 2 {
        return Err(LogicError::InvalidArgumentsError);
    }

    // Evaluate the first argument (minimum number of required fields)
    let min_required = evaluate(args[0], arena)?;
    let min_count = min_required
        .coerce_to_number()
        .map(|n| n.as_i64().unwrap_or(0))
        .unwrap_or(0) as usize;

    // Evaluate the second argument (array of field names)
    let fields = evaluate(args[1], arena)?;

    if let DataValue::Array(names) = fields {
        // Count how many fields are present
        let mut found_count = 0;
        let mut missing = arena.get_data_value_vec();

        for name_value in *names {
            if let DataValue::String(name) = name_value {
                if variable_exists(name, arena) {
                    found_count += 1;
                } else {
                    missing.push(DataValue::String(name));
                }
            }
            // Ignore non-string names
        }

        // If we have enough fields, return an empty array
        if found_count >= min_count {
            return Ok(arena.empty_array_value());
        }

        // Otherwise return the missing fields
        let result = DataValue::Array(arena.bump_vec_into_slice(missing));
        Ok(arena.alloc(result))
    } else {
        // If the second argument is not an array, return an empty array
        Ok(arena.empty_array_value())
    }
}

#[cfg(test)]
mod tests {
    use crate::logic::datalogic_core::DataLogicCore;
    use crate::logic::token::{OperatorType, Token};
    use crate::logic::Logic;
    use crate::value::DataValue;
    use serde_json::json;

    #[test]
    fn test_missing() {
        let core = DataLogicCore::new();
        let arena = core.arena();

        let data = json!({
            "a": 1,
            "c": 3,
        });

        // Test missing with single value
        // Create {"missing": ["b"]}
        let b_str_token = Token::literal(DataValue::string(arena, "b"));
        let b_str_ref = arena.alloc(b_str_token);

        let missing_args = vec![b_str_ref];
        let missing_array_token = Token::ArrayLiteral(missing_args);
        let missing_array_ref = arena.alloc(missing_array_token);

        let missing_token = Token::operator(OperatorType::Missing, missing_array_ref);
        let missing_ref = arena.alloc(missing_token);

        let rule = Logic::new(missing_ref, arena);

        let result = core.apply(&rule, &data).unwrap();
        assert!(result.is_array());
        let arr = result.as_array().unwrap();
        assert_eq!(arr.len(), 1);
        assert_eq!(arr[0].as_str(), Some("b"));

        // Test missing with multiple values
        // Create {"missing": ["a", "b", "c", "d"]}
        let a_str_token = Token::literal(DataValue::string(arena, "a"));
        let a_str_ref = arena.alloc(a_str_token);

        let b_str_token = Token::literal(DataValue::string(arena, "b"));
        let b_str_ref = arena.alloc(b_str_token);

        let c_str_token = Token::literal(DataValue::string(arena, "c"));
        let c_str_ref = arena.alloc(c_str_token);

        let d_str_token = Token::literal(DataValue::string(arena, "d"));
        let d_str_ref = arena.alloc(d_str_token);

        let missing_args = vec![a_str_ref, b_str_ref, c_str_ref, d_str_ref];
        let missing_array_token = Token::ArrayLiteral(missing_args);
        let missing_array_ref = arena.alloc(missing_array_token);

        let missing_token = Token::operator(OperatorType::Missing, missing_array_ref);
        let missing_ref = arena.alloc(missing_token);

        let rule = Logic::new(missing_ref, arena);

        let result = core.apply(&rule, &data).unwrap();
        assert!(result.is_array());
        let arr = result.as_array().unwrap();
        assert_eq!(arr.len(), 2);
        assert_eq!(arr[0].as_str(), Some("b"));
        assert_eq!(arr[1].as_str(), Some("d"));

        // Test missing with empty list
        // Create {"missing": []}
        let empty_missing_args: Vec<&Token> = vec![];
        let empty_missing_array_token = Token::ArrayLiteral(empty_missing_args);
        let empty_missing_array_ref = arena.alloc(empty_missing_array_token);

        let missing_token = Token::operator(OperatorType::Missing, empty_missing_array_ref);
        let missing_ref = arena.alloc(missing_token);

        let rule = Logic::new(missing_ref, arena);

        let result = core.apply(&rule, &data).unwrap();
        assert!(result.is_array());
        let arr = result.as_array().unwrap();
        assert_eq!(arr.len(), 0);

        // Test missing with all present
        // Create {"missing": ["a", "c"]}
        let a_str_token = Token::literal(DataValue::string(arena, "a"));
        let a_str_ref = arena.alloc(a_str_token);

        let c_str_token = Token::literal(DataValue::string(arena, "c"));
        let c_str_ref = arena.alloc(c_str_token);

        let missing_args = vec![a_str_ref, c_str_ref];
        let missing_array_token = Token::ArrayLiteral(missing_args);
        let missing_array_ref = arena.alloc(missing_array_token);

        let missing_token = Token::operator(OperatorType::Missing, missing_array_ref);
        let missing_ref = arena.alloc(missing_token);

        let rule = Logic::new(missing_ref, arena);

        let result = core.apply(&rule, &data).unwrap();
        assert!(result.is_array());
        let arr = result.as_array().unwrap();
        assert_eq!(arr.len(), 0);
    }

    #[test]
    fn test_missing_some() {
        let core = DataLogicCore::new();
        let arena = core.arena();

        let data = json!({
            "a": 1,
            "c": 3,
        });

        // Test missing_some with min_required=1, all missing
        // Create {"missing_some": [1, ["b", "d"]]}
        let one_token = Token::literal(DataValue::integer(1));
        let one_ref = arena.alloc(one_token);

        let b_str_token = Token::literal(DataValue::string(arena, "b"));
        let b_str_ref = arena.alloc(b_str_token);

        let d_str_token = Token::literal(DataValue::string(arena, "d"));
        let d_str_ref = arena.alloc(d_str_token);

        let vars_args = vec![b_str_ref, d_str_ref];
        let vars_array_token = Token::ArrayLiteral(vars_args);
        let vars_array_ref = arena.alloc(vars_array_token);

        let missing_some_args = vec![one_ref, vars_array_ref];
        let missing_some_array_token = Token::ArrayLiteral(missing_some_args);
        let missing_some_array_ref = arena.alloc(missing_some_array_token);

        let missing_some_token = Token::operator(OperatorType::MissingSome, missing_some_array_ref);
        let missing_some_ref = arena.alloc(missing_some_token);

        let rule = Logic::new(missing_some_ref, arena);

        let result = core.apply(&rule, &data).unwrap();
        assert!(result.is_array());
        let arr = result.as_array().unwrap();
        assert_eq!(arr.len(), 2);
        assert_eq!(arr[0].as_str(), Some("b"));
        assert_eq!(arr[1].as_str(), Some("d"));

        // Test missing_some with min_required=1, some present
        // Create {"missing_some": [1, ["a", "b", "c", "d"]]}
        let one_token = Token::literal(DataValue::integer(1));
        let one_ref = arena.alloc(one_token);

        let a_str_token = Token::literal(DataValue::string(arena, "a"));
        let a_str_ref = arena.alloc(a_str_token);

        let b_str_token = Token::literal(DataValue::string(arena, "b"));
        let b_str_ref = arena.alloc(b_str_token);

        let c_str_token = Token::literal(DataValue::string(arena, "c"));
        let c_str_ref = arena.alloc(c_str_token);

        let d_str_token = Token::literal(DataValue::string(arena, "d"));
        let d_str_ref = arena.alloc(d_str_token);

        let vars_args = vec![a_str_ref, b_str_ref, c_str_ref, d_str_ref];
        let vars_array_token = Token::ArrayLiteral(vars_args);
        let vars_array_ref = arena.alloc(vars_array_token);

        let missing_some_args = vec![one_ref, vars_array_ref];
        let missing_some_array_token = Token::ArrayLiteral(missing_some_args);
        let missing_some_array_ref = arena.alloc(missing_some_array_token);

        let missing_some_token = Token::operator(OperatorType::MissingSome, missing_some_array_ref);
        let missing_some_ref = arena.alloc(missing_some_token);

        let rule = Logic::new(missing_some_ref, arena);

        let result = core.apply(&rule, &data).unwrap();
        assert!(result.is_array());
        let arr = result.as_array().unwrap();
        assert_eq!(arr.len(), 0);

        // Test missing_some with min_required=3, only 2 present
        // Create {"missing_some": [3, ["a", "b", "c", "d"]]}
        let three_token = Token::literal(DataValue::integer(3));
        let three_ref = arena.alloc(three_token);

        let a_str_token = Token::literal(DataValue::string(arena, "a"));
        let a_str_ref = arena.alloc(a_str_token);

        let b_str_token = Token::literal(DataValue::string(arena, "b"));
        let b_str_ref = arena.alloc(b_str_token);

        let c_str_token = Token::literal(DataValue::string(arena, "c"));
        let c_str_ref = arena.alloc(c_str_token);

        let d_str_token = Token::literal(DataValue::string(arena, "d"));
        let d_str_ref = arena.alloc(d_str_token);

        let vars_args = vec![a_str_ref, b_str_ref, c_str_ref, d_str_ref];
        let vars_array_token = Token::ArrayLiteral(vars_args);
        let vars_array_ref = arena.alloc(vars_array_token);

        let missing_some_args = vec![three_ref, vars_array_ref];
        let missing_some_array_token = Token::ArrayLiteral(missing_some_args);
        let missing_some_array_ref = arena.alloc(missing_some_array_token);

        let missing_some_token = Token::operator(OperatorType::MissingSome, missing_some_array_ref);
        let missing_some_ref = arena.alloc(missing_some_token);

        let rule = Logic::new(missing_some_ref, arena);

        let result = core.apply(&rule, &data).unwrap();
        assert!(result.is_array());
        let arr = result.as_array().unwrap();
        assert_eq!(arr.len(), 2);
        assert_eq!(arr[0].as_str(), Some("b"));
        assert_eq!(arr[1].as_str(), Some("d"));

        // Test missing_some with min_required=0
        // Create {"missing_some": [0, ["b", "d"]]}
        let zero_token = Token::literal(DataValue::integer(0));
        let zero_ref = arena.alloc(zero_token);

        let b_str_token = Token::literal(DataValue::string(arena, "b"));
        let b_str_ref = arena.alloc(b_str_token);

        let d_str_token = Token::literal(DataValue::string(arena, "d"));
        let d_str_ref = arena.alloc(d_str_token);

        let vars_args = vec![b_str_ref, d_str_ref];
        let vars_array_token = Token::ArrayLiteral(vars_args);
        let vars_array_ref = arena.alloc(vars_array_token);

        let missing_some_args = vec![zero_ref, vars_array_ref];
        let missing_some_array_token = Token::ArrayLiteral(missing_some_args);
        let missing_some_array_ref = arena.alloc(missing_some_array_token);

        let missing_some_token = Token::operator(OperatorType::MissingSome, missing_some_array_ref);
        let missing_some_ref = arena.alloc(missing_some_token);

        let rule = Logic::new(missing_some_ref, arena);

        let result = core.apply(&rule, &data).unwrap();
        assert!(result.is_array());
        let arr = result.as_array().unwrap();
        assert_eq!(arr.len(), 0);
    }
}
