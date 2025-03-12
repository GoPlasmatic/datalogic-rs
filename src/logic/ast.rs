//! Abstract Syntax Tree for logic expressions.
//!
//! This module provides the Logic struct, which represents a logic expression
//! as an Abstract Syntax Tree (AST).

use crate::arena::DataArena;
use crate::value::DataValue;
use super::token::{Token, OperatorType};

/// A logic expression.
///
/// This struct represents a logic expression as an Abstract Syntax Tree (AST).
/// It holds a reference to the root token of the expression and the arena
/// in which the tokens are allocated.
#[derive(Debug, Clone)]
pub struct Logic<'a> {
    /// The root token of the logic expression.
    root: &'a Token<'a>,
    
    /// The arena in which the tokens are allocated.
    arena: &'a DataArena,
}

impl<'a> Logic<'a> {
    /// Creates a new logic expression.
    pub fn new(root: &'a Token<'a>, arena: &'a DataArena) -> Self {
        Self { root, arena }
    }
    
    /// Creates a new logic expression from a token.
    pub fn from_token(token: Token<'a>, arena: &'a DataArena) -> Self {
        let root = arena.alloc(token);
        Self { root, arena }
    }
    
    /// Returns the root token of the logic expression.
    pub fn root(&self) -> &'a Token<'a> {
        self.root
    }
    
    /// Returns the arena in which the tokens are allocated.
    pub fn arena(&self) -> &'a DataArena {
        self.arena
    }
    
    /// Creates a new literal logic expression.
    pub fn literal(value: DataValue<'a>, arena: &'a DataArena) -> Self {
        let token = Token::literal(value);
        Self::from_token(token, arena)
    }
    
    /// Creates a new variable logic expression.
    pub fn variable(path: &str, default: Option<Logic<'a>>, arena: &'a DataArena) -> Self {
        let path_str = arena.intern_str(path);
        let default_token = default.map(|d| d.root);
        let token = Token::variable(path_str, default_token);
        Self::from_token(token, arena)
    }
    
    /// Creates an operator logic expression.
    pub fn operator(op_type: OperatorType, args: Vec<Logic<'a>>, arena: &'a DataArena) -> Self {
        // Extract the root tokens from the arguments
        let mut arg_tokens = Vec::with_capacity(args.len());
        for arg in &args {
            let token_ref = arg.root;
            arg_tokens.push(token_ref);
        }
        
        // Allocate the argument tokens in the arena
        let array_literal = Token::ArrayLiteral(arg_tokens);
        let array_token = arena.alloc(array_literal);
        
        // Create the operator token
        let token = Token::operator(op_type, array_token);
        
        Self::from_token(token, arena)
    }
    
    /// Creates a custom operator logic expression.
    pub fn custom_operator(name: &str, args: Vec<Logic<'a>>, arena: &'a DataArena) -> Self {
        // Extract the root tokens from the arguments
        let mut arg_tokens = Vec::with_capacity(args.len());
        for arg in &args {
            let token_ref = arg.root;
            arg_tokens.push(token_ref);
        }
        
        // Allocate the argument tokens in the arena
        let array_literal = Token::ArrayLiteral(arg_tokens);
        let array_token = arena.alloc(array_literal);
        
        // Create the custom operator token
        let name_str = arena.intern_str(name);
        let token = Token::custom_operator(name_str, array_token);
        
        Self::from_token(token, arena)
    }
    
    /// Returns true if this logic expression is a literal.
    pub fn is_literal(&self) -> bool {
        self.root.is_literal()
    }
    
    /// Returns true if this logic expression is a variable.
    pub fn is_variable(&self) -> bool {
        self.root.is_variable()
    }
    
    /// Returns true if this logic expression is an operator.
    pub fn is_operator(&self) -> bool {
        self.root.is_operator()
    }
    
    /// Returns true if this logic expression is a custom operator.
    pub fn is_custom_operator(&self) -> bool {
        self.root.is_custom_operator()
    }
    
    /// Returns the literal value if this logic expression is a literal.
    pub fn as_literal(&self) -> Option<&DataValue<'a>> {
        self.root.as_literal()
    }
    
    /// Returns the variable path if this logic expression is a variable.
    pub fn as_variable(&self) -> Option<(&'a str, Option<&'a Token<'a>>)> {
        self.root.as_variable()
    }
    
    /// Returns the operator type and arguments if this logic expression is an operator.
    pub fn as_operator(&self) -> Option<(OperatorType, &'a Token<'a>)> {
        self.root.as_operator()
    }
    
    /// Returns the custom operator name and arguments if this logic expression is a custom operator.
    pub fn as_custom_operator(&self) -> Option<(&'a str, &'a Token<'a>)> {
        self.root.as_custom_operator()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::value::DataValue;
    use crate::logic::operators::comparison::ComparisonOp;
    use crate::logic::operators::logical::LogicalOp;
    
    #[test]
    fn test_logic_creation() {
        let arena = DataArena::new();
        
        // Create a simple logic with a literal
        let logic = Logic::literal(DataValue::integer(42), &arena);
        assert!(logic.is_literal());
        assert_eq!(logic.as_literal().unwrap().as_i64(), Some(42));
    }
    
    #[test]
    fn test_logic_with_default() {
        let arena = DataArena::new();
        
        // Create a variable with a default
        let default = Logic::literal(DataValue::integer(42), &arena);
        let var = Logic::variable("a", Some(default), &arena);
        
        assert!(var.is_variable());
        let (path, default_token) = var.as_variable().unwrap();
        assert_eq!(path, "a");
        assert!(default_token.is_some());
    }
    
    #[test]
    fn test_custom_operator() {
        let arena = DataArena::new();
        
        // Create arguments
        let arg1 = Logic::literal(DataValue::integer(1), &arena);
        let arg2 = Logic::literal(DataValue::integer(2), &arena);
        
        // Create a custom operator
        let logic = Logic::custom_operator("my_op", vec![arg1, arg2], &arena);
        
        assert!(logic.is_custom_operator());
        let (name, args) = logic.as_custom_operator().unwrap();
        assert_eq!(name, "my_op");
        
        // Check that args is an ArrayLiteral
        assert!(args.is_array_literal());
    }
    
    #[test]
    fn test_comparison_operator() {
        let arena = DataArena::new();
        
        // Create arguments
        let arg1 = Logic::variable("a", None, &arena);
        let arg2 = Logic::literal(DataValue::integer(42), &arena);
        
        // Create a comparison operator
        let logic = Logic::operator(OperatorType::Comparison(ComparisonOp::Equal), vec![arg1, arg2], &arena);
        
        assert!(logic.is_operator());
        let (op_type, args) = logic.as_operator().unwrap();
        assert_eq!(op_type, OperatorType::Comparison(ComparisonOp::Equal));
        
        // Check that args is an ArrayLiteral
        assert!(args.is_array_literal());
    }
    
    #[test]
    fn test_logical_operator() {
        let arena = DataArena::new();
        
        // Create arguments
        let arg1 = Logic::literal(DataValue::bool(true), &arena);
        let arg2 = Logic::literal(DataValue::bool(false), &arena);
        
        // Create a logical operator
        let logic = Logic::operator(OperatorType::Logical(LogicalOp::And), vec![arg1, arg2], &arena);
        
        assert!(logic.is_operator());
        let (op_type, args) = logic.as_operator().unwrap();
        assert_eq!(op_type, OperatorType::Logical(LogicalOp::And));
        
        // Check that args is an ArrayLiteral
        assert!(args.is_array_literal());
    }
} 