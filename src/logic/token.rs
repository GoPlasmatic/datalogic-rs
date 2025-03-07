//! Token representation for logic expressions.
//!
//! This module provides a compact token representation for logic expressions,
//! optimized for memory efficiency and evaluation performance.

use crate::value::DataValue;
use super::operators::{ComparisonOp, ArithmeticOp, LogicalOp, StringOp, ArrayOp, ConditionalOp};

/// A token in a logic expression.
///
/// This is a compact representation of a logic expression node, optimized
/// for memory efficiency and evaluation performance.
#[derive(Debug, Clone, PartialEq)]
pub enum Token<'a> {
    /// A literal value.
    Literal(DataValue<'a>),
    
    /// A variable reference.
    Variable {
        /// The path to the variable.
        path: &'a str,
        /// An optional default value if the variable is not found.
        default: Option<&'a Token<'a>>,
    },
    
    /// A variable reference with a dynamic path.
    DynamicVariable {
        /// The token that evaluates to the path.
        path_expr: &'a Token<'a>,
        /// An optional default value if the variable is not found.
        default: Option<&'a Token<'a>>,
    },
    
    /// An operator application.
    Operator {
        /// The type of operator.
        op_type: OperatorType,
        /// The arguments to the operator.
        args: &'a [Token<'a>],
    },
    
    /// A custom operator application.
    CustomOperator {
        /// The name of the custom operator.
        name: &'a str,
        /// The arguments to the operator.
        args: &'a [Token<'a>],
    },
}

/// The type of operator.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OperatorType {
    /// Comparison operator
    Comparison(ComparisonOp),
    /// Arithmetic operator
    Arithmetic(ArithmeticOp),
    /// Logical operator
    Logical(LogicalOp),
    /// String operator
    String(StringOp),
    /// Array operator
    Array(ArrayOp),
    /// Conditional operator
    Conditional(ConditionalOp),
    /// Log operator
    Log,
    /// In operator
    In,
    /// Missing operator
    Missing,
    /// Missing Some operator
    MissingSome,
    /// Array operator (for arrays with non-literal elements)
    ArrayLiteral,
}

impl<'a> Token<'a> {
    /// Creates a new literal token.
    pub fn literal(value: DataValue<'a>) -> Self {
        Token::Literal(value)
    }
    
    /// Creates a new variable token.
    pub fn variable(path: &'a str, default: Option<&'a Token<'a>>) -> Self {
        Token::Variable { path, default }
    }
    
    /// Creates a new dynamic variable token.
    pub fn dynamic_variable(path_expr: &'a Token<'a>, default: Option<&'a Token<'a>>) -> Self {
        Token::DynamicVariable { path_expr, default }
    }
    
    /// Creates a new operator token.
    pub fn operator(op_type: OperatorType, args: &'a [Token<'a>]) -> Self {
        Token::Operator { op_type, args }
    }
    
    /// Creates a new custom operator token.
    pub fn custom_operator(name: &'a str, args: &'a [Token<'a>]) -> Self {
        Token::CustomOperator { name, args }
    }
    
    /// Returns true if this token is a literal.
    pub fn is_literal(&self) -> bool {
        matches!(self, Token::Literal(_))
    }
    
    /// Returns true if this token is a variable.
    pub fn is_variable(&self) -> bool {
        matches!(self, Token::Variable { .. })
    }
    
    /// Returns true if this token is an operator.
    pub fn is_operator(&self) -> bool {
        matches!(self, Token::Operator { .. })
    }
    
    /// Returns true if this token is a custom operator.
    pub fn is_custom_operator(&self) -> bool {
        matches!(self, Token::CustomOperator { .. })
    }
    
    /// Returns the literal value if this token is a literal.
    pub fn as_literal(&self) -> Option<&DataValue<'a>> {
        match self {
            Token::Literal(value) => Some(value),
            _ => None,
        }
    }
    
    /// Returns the variable path if this token is a variable.
    pub fn as_variable(&self) -> Option<(&'a str, Option<&'a Token<'a>>)> {
        match self {
            Token::Variable { path, default } => Some((path, *default)),
            _ => None,
        }
    }
    
    /// Returns the operator type and arguments if this token is an operator.
    pub fn as_operator(&self) -> Option<(OperatorType, &'a [Token<'a>])> {
        match self {
            Token::Operator { op_type, args } => Some((*op_type, args)),
            _ => None,
        }
    }
    
    /// Returns the custom operator name and arguments if this token is a custom operator.
    pub fn as_custom_operator(&self) -> Option<(&'a str, &'a [Token<'a>])> {
        match self {
            Token::CustomOperator { name, args } => Some((name, args)),
            _ => None,
        }
    }
}

impl OperatorType {
    /// Returns the string representation of this operator type.
    pub fn as_str(&self) -> &'static str {
        match self {
            OperatorType::Comparison(op) => match op {
                ComparisonOp::Equal => "==",
                ComparisonOp::StrictEqual => "===",
                ComparisonOp::NotEqual => "!=",
                ComparisonOp::StrictNotEqual => "!==",
                ComparisonOp::GreaterThan => ">",
                ComparisonOp::GreaterThanOrEqual => ">=",
                ComparisonOp::LessThan => "<",
                ComparisonOp::LessThanOrEqual => "<=",
            },
            OperatorType::Arithmetic(op) => match op {
                ArithmeticOp::Add => "+",
                ArithmeticOp::Subtract => "-",
                ArithmeticOp::Multiply => "*",
                ArithmeticOp::Divide => "/",
                ArithmeticOp::Modulo => "%",
                ArithmeticOp::Min => "min",
                ArithmeticOp::Max => "max",
            },
            OperatorType::Logical(op) => match op {
                LogicalOp::And => "and",
                LogicalOp::Or => "or",
                LogicalOp::Not => "!",
            },
            OperatorType::String(op) => match op {
                StringOp::Cat => "cat",
                StringOp::Substr => "substr",
            },
            OperatorType::Array(op) => match op {
                ArrayOp::Map => "map",
                ArrayOp::Filter => "filter",
                ArrayOp::Reduce => "reduce",
                ArrayOp::All => "all",
                ArrayOp::Some => "some",
                ArrayOp::None => "none",
                ArrayOp::Merge => "merge",
            },
            OperatorType::Conditional(op) => match op {
                ConditionalOp::If => "if",
                ConditionalOp::Ternary => "?:",
            },
            OperatorType::Log => "log",
            OperatorType::In => "in",
            OperatorType::Missing => "missing",
            OperatorType::MissingSome => "missing_some",
            OperatorType::ArrayLiteral => "array",
        }
    }
    
    /// Returns the operator type for the given string, if it exists.
    pub fn from_str(s: &str) -> Option<Self> {
        match s {
            "==" => Some(OperatorType::Comparison(ComparisonOp::Equal)),
            "===" => Some(OperatorType::Comparison(ComparisonOp::StrictEqual)),
            "!=" => Some(OperatorType::Comparison(ComparisonOp::NotEqual)),
            "!==" => Some(OperatorType::Comparison(ComparisonOp::StrictNotEqual)),
            ">" => Some(OperatorType::Comparison(ComparisonOp::GreaterThan)),
            ">=" => Some(OperatorType::Comparison(ComparisonOp::GreaterThanOrEqual)),
            "<" => Some(OperatorType::Comparison(ComparisonOp::LessThan)),
            "<=" => Some(OperatorType::Comparison(ComparisonOp::LessThanOrEqual)),
            "+" => Some(OperatorType::Arithmetic(ArithmeticOp::Add)),
            "-" => Some(OperatorType::Arithmetic(ArithmeticOp::Subtract)),
            "*" => Some(OperatorType::Arithmetic(ArithmeticOp::Multiply)),
            "/" => Some(OperatorType::Arithmetic(ArithmeticOp::Divide)),
            "%" => Some(OperatorType::Arithmetic(ArithmeticOp::Modulo)),
            "min" => Some(OperatorType::Arithmetic(ArithmeticOp::Min)),
            "max" => Some(OperatorType::Arithmetic(ArithmeticOp::Max)),
            "and" => Some(OperatorType::Logical(LogicalOp::And)),
            "or" => Some(OperatorType::Logical(LogicalOp::Or)),
            "!" => Some(OperatorType::Logical(LogicalOp::Not)),
            "cat" => Some(OperatorType::String(StringOp::Cat)),
            "substr" => Some(OperatorType::String(StringOp::Substr)),
            "map" => Some(OperatorType::Array(ArrayOp::Map)),
            "filter" => Some(OperatorType::Array(ArrayOp::Filter)),
            "reduce" => Some(OperatorType::Array(ArrayOp::Reduce)),
            "all" => Some(OperatorType::Array(ArrayOp::All)),
            "some" => Some(OperatorType::Array(ArrayOp::Some)),
            "none" => Some(OperatorType::Array(ArrayOp::None)),
            "merge" => Some(OperatorType::Array(ArrayOp::Merge)),
            "if" => Some(OperatorType::Conditional(ConditionalOp::If)),
            "?:" => Some(OperatorType::Conditional(ConditionalOp::Ternary)),
            "log" => Some(OperatorType::Log),
            "in" => Some(OperatorType::In),
            "missing" => Some(OperatorType::Missing),
            "missing_some" => Some(OperatorType::MissingSome),
            "array" => Some(OperatorType::ArrayLiteral),
            _ => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    
    use crate::arena::DataArena;

    #[test]
    fn test_token_creation() {
        let arena = DataArena::new();
        
        // Create a literal token
        let literal = Token::literal(DataValue::integer(42));
        assert!(literal.is_literal());
        assert_eq!(literal.as_literal().unwrap().as_i64(), Some(42));
        
        // Create a variable token
        let variable = Token::variable(arena.intern_str("user.name"), None);
        assert!(variable.is_variable());
        let (path, default) = variable.as_variable().unwrap();
        assert_eq!(path, "user.name");
        assert!(default.is_none());
        
        // Create an operator token
        let args = arena.alloc_slice_clone(&[
            Token::literal(DataValue::integer(1)),
            Token::literal(DataValue::integer(2)),
        ]);
        let operator = Token::operator(OperatorType::Comparison(ComparisonOp::Equal), args);
        assert!(operator.is_operator());
        let (op_type, op_args) = operator.as_operator().unwrap();
        assert_eq!(op_type, OperatorType::Comparison(ComparisonOp::Equal));
        assert_eq!(op_args.len(), 2);
        assert_eq!(op_args[0].as_literal().unwrap().as_i64(), Some(1));
        assert_eq!(op_args[1].as_literal().unwrap().as_i64(), Some(2));
    }

    #[test]
    fn test_operator_type_conversion() {
        assert_eq!(OperatorType::Comparison(ComparisonOp::Equal).as_str(), "==");
        assert_eq!(OperatorType::from_str("=="), Some(OperatorType::Comparison(ComparisonOp::Equal)));
        
        assert_eq!(OperatorType::Arithmetic(ArithmeticOp::Add).as_str(), "+");
        assert_eq!(OperatorType::from_str("+"), Some(OperatorType::Arithmetic(ArithmeticOp::Add)));
        
        assert_eq!(OperatorType::Logical(LogicalOp::And).as_str(), "and");
        assert_eq!(OperatorType::from_str("and"), Some(OperatorType::Logical(LogicalOp::And)));
        
        assert_eq!(OperatorType::from_str("unknown"), None);
    }
} 