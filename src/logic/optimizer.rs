//! Optimizer for logic expressions.
//!
//! This module provides functions for optimizing logic expressions by
//! precomputing static parts of the expression at compile time.

use crate::arena::DataArena;
use super::token::Token;
use super::error::Result;
use super::evaluator::evaluate;
use super::token::OperatorType;

/// Optimizes a logic expression by precomputing static parts.
///
/// This function traverses the logic expression tree and identifies subexpressions
/// that can be evaluated at compile time (i.e., they don't depend on the input data).
/// These subexpressions are replaced with their computed values, which can significantly
/// improve evaluation performance at runtime.
pub fn optimize<'a>(token: &'a Token<'a>, arena: &'a DataArena) -> Result<&'a Token<'a>> {
    match token {
        // Literals are already optimized
        Token::Literal(_) => Ok(token),
        
        // Variables can't be optimized further
        Token::Variable { .. } => Ok(token),
        
        // Dynamic variables can't be optimized further
        Token::DynamicVariable { .. } => Ok(token),
        
        // Operators might be optimizable if all their arguments are static
        Token::Operator { op_type, args } => {
            // Special case: missing and missing_some operators always need to access data
            // so they should not be statically optimized even if their arguments are static
            if *op_type == OperatorType::Missing || *op_type == OperatorType::MissingSome {
                // Still optimize the arguments
                let mut optimized_args = Vec::with_capacity(args.len());
                
                for arg in *args {
                    let opt_arg = optimize(arg, arena)?;
                    
                    // Clone the token to get ownership
                    let token_clone = arena.alloc(opt_arg.clone());
                    optimized_args.push(token_clone.clone());
                }
                
                // Create a new operator with the optimized arguments
                let optimized_args_slice = arena.alloc_slice_clone(&optimized_args);
                return Ok(arena.alloc(Token::operator(*op_type, optimized_args_slice)));
            }
            
            // For other operators, proceed with normal optimization
            let mut optimized_args = Vec::with_capacity(args.len());
            let mut all_static = true;
            
            for arg in *args {
                let opt_arg = optimize(arg, arena)?;
                
                // Check if the argument is static (i.e., a literal)
                if !opt_arg.is_literal() {
                    all_static = false;
                }
                
                // Clone the token to get ownership
                let token_clone = arena.alloc(opt_arg.clone());
                optimized_args.push(token_clone.clone());
            }
            
            // If all arguments are static, we can evaluate the operator at compile time
            if all_static {
                // Create a dummy data value for evaluation
                // Use the null_value method to get a reference to a null value
                let dummy_data = arena.null_value();
                
                // Create a new token with the optimized arguments
                let optimized_args_slice = arena.alloc_slice_clone(&optimized_args);
                let new_token = Token::operator(*op_type, optimized_args_slice);
                let new_token_ref = arena.alloc(new_token);
                
                // Try to evaluate the operator
                match evaluate(new_token_ref, dummy_data, arena) {
                    Ok(result) => {
                        // Replace the operator with its computed value
                        Ok(arena.alloc(Token::literal(result.clone())))
                    },
                    Err(_) => {
                        // If evaluation fails, just return the optimized operator
                        Ok(new_token_ref)
                    }
                }
            } else {
                // If not all arguments are static, create a new operator with the optimized arguments
                let optimized_args_slice = arena.alloc_slice_clone(&optimized_args);
                Ok(arena.alloc(Token::operator(*op_type, optimized_args_slice)))
            }
        },
        
        // Custom operators might be optimizable if all their arguments are static
        Token::CustomOperator { name, args } => {
            // Optimize each argument
            let mut optimized_args = Vec::with_capacity(args.len());
            
            for arg in *args {
                let opt_arg = optimize(arg, arena)?;
                
                // Clone the token to get ownership
                let token_clone = arena.alloc(opt_arg.clone());
                optimized_args.push(token_clone.clone());
            }
            
            // Custom operators can't be evaluated at compile time in general,
            // so just return the optimized operator
            let optimized_args_slice = arena.alloc_slice_clone(&optimized_args);
            Ok(arena.alloc(Token::custom_operator(name, optimized_args_slice)))
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::logic::operators::comparison::ComparisonOp;
    use crate::logic::operators::logical::LogicalOp;
    use crate::logic::operators::arithmetic::ArithmeticOp;
    use crate::logic::parse_str;

    #[test]
    fn test_optimize_literals() {
        let arena = DataArena::new();
        
        // Parse a simple literal
        let token = parse_str("42", &arena).unwrap();
        let optimized = optimize(token, &arena).unwrap();
        
        // Should be unchanged
        assert!(optimized.is_literal());
        assert_eq!(optimized.as_literal().unwrap().as_i64(), Some(42));
    }
    
    #[test]
    fn test_optimize_variables() {
        let arena = DataArena::new();
        
        // Parse a simple variable
        let token = parse_str(r#"{"var": "a"}"#, &arena).unwrap();
        let optimized = optimize(token, &arena).unwrap();
        
        // Should be unchanged
        assert!(optimized.is_variable());
        let (path, _) = optimized.as_variable().unwrap();
        assert_eq!(path, "a");
    }
    
    #[test]
    fn test_optimize_static_operator() {
        let arena = DataArena::new();
        
        // Parse a static operator (1 + 2)
        let token = parse_str(r#"{"+":[1,2]}"#, &arena).unwrap();
        let optimized = optimize(token, &arena).unwrap();
        
        // Should be optimized to a literal
        assert!(optimized.is_literal());
        assert_eq!(optimized.as_literal().unwrap().as_i64(), Some(3));
    }
    
    #[test]
    fn test_optimize_mixed_operator() {
        let arena = DataArena::new();
        
        // Parse a mixed operator (1 + var)
        let token = parse_str(r#"{"+":[1,{"var":"a"}]}"#, &arena).unwrap();
        let optimized = optimize(token, &arena).unwrap();
        
        // Should still be an operator
        assert!(optimized.is_operator());
        let (op_type, args) = optimized.as_operator().unwrap();
        assert_eq!(op_type, OperatorType::Arithmetic(ArithmeticOp::Add));
        assert_eq!(args.len(), 2);
        
        // First argument should be optimized to a literal
        assert!(args[0].is_literal());
        assert_eq!(args[0].as_literal().unwrap().as_i64(), Some(1));
        
        // Second argument should still be a variable
        assert!(args[1].is_variable());
    }
    
    #[test]
    fn test_optimize_nested_operators() {
        let arena = DataArena::new();
        
        // Parse a nested operator ((1 + 2) * 3)
        let token = parse_str(r#"{"*":[{"+": [1,2]},3]}"#, &arena).unwrap();
        let optimized = optimize(token, &arena).unwrap();
        
        // Should be optimized to a literal
        assert!(optimized.is_literal());
        assert_eq!(optimized.as_literal().unwrap().as_i64(), Some(9));
    }
    
    #[test]
    fn test_optimize_complex_expression() {
        let arena = DataArena::new();
        
        // Parse a complex expression with both static and dynamic parts
        let token = parse_str(
            r#"{"and":[{"==":[{"var":"a"},5]},{"==":[{"+": [1,2]},3]}]}"#, 
            &arena
        ).unwrap();
        let optimized = optimize(token, &arena).unwrap();
        
        // Should still be an operator
        assert!(optimized.is_operator());
        let (op_type, args) = optimized.as_operator().unwrap();
        assert_eq!(op_type, OperatorType::Logical(LogicalOp::And));
        assert_eq!(args.len(), 2);
        
        // First argument should still be a comparison
        assert!(args[0].is_operator());
        let (comp_op, _comp_args) = args[0].as_operator().unwrap();
        assert_eq!(comp_op, OperatorType::Comparison(ComparisonOp::Equal));
        
        // Second argument should be optimized to a literal
        assert!(args[1].is_literal());
        assert_eq!(args[1].as_literal().unwrap().as_bool(), Some(true));
    }
    
    #[test]
    fn test_missing_operator_not_optimized() {
        let arena = DataArena::new();
        
        // Parse a missing operator with static arguments
        let token = parse_str(r#"{"missing":["a","b"]}"#, &arena).unwrap();
        let optimized = optimize(token, &arena).unwrap();
        
        // Should still be an operator, not optimized to a literal
        assert!(optimized.is_operator());
        let (op_type, args) = optimized.as_operator().unwrap();
        assert_eq!(op_type, OperatorType::Missing);
        
        // Arguments should be optimized
        assert_eq!(args.len(), 2);
        assert!(args[0].is_literal());
        assert!(args[1].is_literal());
    }
    
    #[test]
    fn test_missing_some_operator_not_optimized() {
        let arena = DataArena::new();
        
        // Parse a missing_some operator with static arguments
        let token = parse_str(r#"{"missing_some":[1,["a","b","c"]]}"#, &arena).unwrap();
        let optimized = optimize(token, &arena).unwrap();
        
        // Should still be an operator, not optimized to a literal
        assert!(optimized.is_operator());
        let (op_type, args) = optimized.as_operator().unwrap();
        assert_eq!(op_type, OperatorType::MissingSome);
        
        // Arguments should be optimized
        assert_eq!(args.len(), 2);
        assert!(args[0].is_literal());
        assert!(args[1].is_literal());
    }
} 