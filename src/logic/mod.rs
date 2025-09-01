//! Logic expression representation and evaluation.
//!
//! This module provides types and functions for representing and evaluating
//! logic expressions using arena allocation for improved performance.

mod ast;
mod datalogic_core;
pub mod error;
mod evaluator;
mod operators;
mod optimizer;
pub mod token;

pub use ast::Logic;
pub use datalogic_core::DataLogicCore;
pub use error::{LogicError, Result};
pub use evaluator::evaluate;
pub use token::{OperatorType, Token};

// Re-export operator types
pub use operators::arithmetic::ArithmeticOp;
pub use operators::array::ArrayOp;
pub use operators::comparison::ComparisonOp;
pub use operators::control::ControlOp;
pub use operators::datetime::DateTimeOp;
pub use operators::string::StringOp;

/// Make optimizer function public
pub fn optimize<'a>(
    token: &'a Token<'a>,
    arena: &'a crate::arena::DataArena,
) -> Result<&'a Token<'a>> {
    optimizer::optimize(token, arena)
}

// Implement IntoLogic for common types is now handled through the DataLogic interface

#[cfg(test)]
mod tests {
    use super::*;
    use crate::arena::DataArena;
    use crate::parser::jsonlogic;
    use crate::value::{DataValue, FromJson};
    use serde_json::json;

    #[test]
    fn test_simple_logic() {
        let arena = DataArena::new();

        // Create a simple comparison logic
        let rule_json = json!({"==": [{"var": "a"}, 10]});

        // Use the parser from the parser module
        let token = jsonlogic::parse_json(&rule_json, &arena).unwrap();
        let logic = Logic::new(token, &arena);

        // Create test data
        let data_json = json!({"a": 10});
        let data = DataValue::from_json(&data_json, &arena);
        let data_ref = arena.alloc(data);
        let context = crate::context::EvalContext::new(data_ref);
        // Evaluate logic
        let result = evaluate(logic.root(), &context, &arena).unwrap();

        // Verify result
        assert_eq!(result.as_bool(), Some(true));
    }

    #[test]
    fn test_optimized_logic() {
        let arena = DataArena::new();

        // Create a logic with static parts that can be optimized
        let rule_json = json!({"and": [
            {"==": [{"var": "a"}, 10]},
            {"==": [{"+":[1, 2]}, 3]}
        ]});

        // Use the parser from the parser module
        let token = jsonlogic::parse_json(&rule_json, &arena).unwrap();
        let optimized_token = optimizer::optimize(token, &arena).unwrap();
        let logic = Logic::new(optimized_token, &arena);

        // The second part of the AND should be optimized to a literal true
        let (op_type, args) = logic.root().as_operator().unwrap();
        assert_eq!(op_type, OperatorType::Control(ControlOp::And));

        // Check that args is an ArrayLiteral
        assert!(args.is_array_literal());
        let array_tokens = args.as_array_literal().unwrap();
        assert_eq!(array_tokens.len(), 2);

        // First argument should still be a comparison
        assert!(array_tokens[0].is_operator());

        // Second argument should be optimized to a literal
        assert!(array_tokens[1].is_literal());
        assert_eq!(array_tokens[1].as_literal().unwrap().as_bool(), Some(true));

        // Create test data
        let data_json = json!({"a": 10});
        let data = DataValue::from_json(&data_json, &arena);
        let data_ref = arena.alloc(data);
        let context = crate::context::EvalContext::new(data_ref);
        // Evaluate logic
        let result = evaluate(logic.root(), &context, &arena).unwrap();

        // Verify result
        assert_eq!(result.as_bool(), Some(true));
    }
}
