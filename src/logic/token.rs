//! Token representation for logic expressions.
//!
//! This module provides a compact token representation for logic expressions,
//! optimized for memory efficiency and evaluation performance.

use super::operators::{ArithmeticOp, ArrayOp, ComparisonOp, ControlOp, StringOp};
use crate::value::DataValue;
use std::str::FromStr;

/// A token in a logic expression.
///
/// This is a compact representation of a logic expression node, optimized
/// for memory efficiency and evaluation performance.
#[derive(Debug, Clone, PartialEq)]
pub enum Token<'a> {
    /// A literal value.
    Literal(DataValue<'a>),

    /// An array literal.
    ArrayLiteral(Vec<&'a Token<'a>>),

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
        args: &'a Token<'a>,
    },

    /// A custom operator application.
    CustomOperator {
        /// The name of the custom operator.
        name: &'a str,
        /// The arguments to the operator.
        args: &'a Token<'a>,
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
    Control(ControlOp),
    /// String operator
    String(StringOp),
    /// Array operator
    Array(ArrayOp),
    /// Log operator
    Log,
    /// Missing operator
    Missing,
    /// Missing Some operator
    MissingSome,
    /// Exists operator
    Exists,
    /// Coalesce operator
    Coalesce,
    /// Val operator (replacement for Var)
    Val,
    /// Throw operator
    Throw,
    /// Try operator (for error handling)
    Try,
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
    pub fn operator(op_type: OperatorType, args: &'a Token<'a>) -> Self {
        Token::Operator { op_type, args }
    }

    /// Creates a new custom operator token.
    pub fn custom_operator(name: &'a str, args: &'a Token<'a>) -> Self {
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

    /// Returns true if this token is an array literal.
    pub fn is_array_literal(&self) -> bool {
        matches!(self, Token::ArrayLiteral(_))
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
    pub fn as_operator(&self) -> Option<(OperatorType, &'a Token<'a>)> {
        match self {
            Token::Operator { op_type, args } => Some((*op_type, args)),
            _ => None,
        }
    }

    /// Returns the custom operator name and arguments if this token is a custom operator.
    pub fn as_custom_operator(&self) -> Option<(&'a str, &'a Token<'a>)> {
        match self {
            Token::CustomOperator { name, args } => Some((name, args)),
            _ => None,
        }
    }

    /// Returns the array tokens if this token is an array literal.
    pub fn as_array_literal(&self) -> Option<&Vec<&'a Token<'a>>> {
        match self {
            Token::ArrayLiteral(tokens) => Some(tokens),
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
            OperatorType::Control(op) => match op {
                ControlOp::If => "if",
                ControlOp::And => "and",
                ControlOp::Or => "or",
                ControlOp::Not => "!",
                ControlOp::DoubleNegation => "!!",
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
                ArrayOp::In => "in",
            },
            OperatorType::Log => "log",
            OperatorType::Missing => "missing",
            OperatorType::MissingSome => "missing_some",
            OperatorType::Exists => "exists",
            OperatorType::Coalesce => "??",
            OperatorType::Val => "val",
            OperatorType::Throw => "throw",
            OperatorType::Try => "try",
            OperatorType::ArrayLiteral => "array",
        }
    }
}

impl FromStr for OperatorType {
    type Err = &'static str; // Or use a more descriptive error type

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "==" => Ok(OperatorType::Comparison(ComparisonOp::Equal)),
            "===" => Ok(OperatorType::Comparison(ComparisonOp::StrictEqual)),
            "!=" => Ok(OperatorType::Comparison(ComparisonOp::NotEqual)),
            "!==" => Ok(OperatorType::Comparison(ComparisonOp::StrictNotEqual)),
            ">" => Ok(OperatorType::Comparison(ComparisonOp::GreaterThan)),
            ">=" => Ok(OperatorType::Comparison(ComparisonOp::GreaterThanOrEqual)),
            "<" => Ok(OperatorType::Comparison(ComparisonOp::LessThan)),
            "<=" => Ok(OperatorType::Comparison(ComparisonOp::LessThanOrEqual)),
            "+" => Ok(OperatorType::Arithmetic(ArithmeticOp::Add)),
            "-" => Ok(OperatorType::Arithmetic(ArithmeticOp::Subtract)),
            "*" => Ok(OperatorType::Arithmetic(ArithmeticOp::Multiply)),
            "/" => Ok(OperatorType::Arithmetic(ArithmeticOp::Divide)),
            "%" => Ok(OperatorType::Arithmetic(ArithmeticOp::Modulo)),
            "min" => Ok(OperatorType::Arithmetic(ArithmeticOp::Min)),
            "max" => Ok(OperatorType::Arithmetic(ArithmeticOp::Max)),
            "and" => Ok(OperatorType::Control(ControlOp::And)),
            "or" => Ok(OperatorType::Control(ControlOp::Or)),
            "!" => Ok(OperatorType::Control(ControlOp::Not)),
            "!!" => Ok(OperatorType::Control(ControlOp::DoubleNegation)),
            "if" => Ok(OperatorType::Control(ControlOp::If)),
            "?:" => Ok(OperatorType::Control(ControlOp::If)),
            "cat" => Ok(OperatorType::String(StringOp::Cat)),
            "substr" => Ok(OperatorType::String(StringOp::Substr)),
            "map" => Ok(OperatorType::Array(ArrayOp::Map)),
            "filter" => Ok(OperatorType::Array(ArrayOp::Filter)),
            "reduce" => Ok(OperatorType::Array(ArrayOp::Reduce)),
            "all" => Ok(OperatorType::Array(ArrayOp::All)),
            "some" => Ok(OperatorType::Array(ArrayOp::Some)),
            "none" => Ok(OperatorType::Array(ArrayOp::None)),
            "merge" => Ok(OperatorType::Array(ArrayOp::Merge)),
            "in" => Ok(OperatorType::Array(ArrayOp::In)),
            "log" => Ok(OperatorType::Log),
            "missing" => Ok(OperatorType::Missing),
            "missing_some" => Ok(OperatorType::MissingSome),
            "exists" => Ok(OperatorType::Exists),
            "??" => Ok(OperatorType::Coalesce),
            "val" => Ok(OperatorType::Val),
            "throw" => Ok(OperatorType::Throw),
            "try" => Ok(OperatorType::Try),
            _ => Err("unknown operator"),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_operator_type_conversion() {
        assert_eq!(OperatorType::Comparison(ComparisonOp::Equal).as_str(), "==");
        assert_eq!(
            OperatorType::from_str("=="),
            Ok(OperatorType::Comparison(ComparisonOp::Equal))
        );

        assert_eq!(OperatorType::Arithmetic(ArithmeticOp::Add).as_str(), "+");
        assert_eq!(
            OperatorType::from_str("+"),
            Ok(OperatorType::Arithmetic(ArithmeticOp::Add))
        );

        assert_eq!(OperatorType::Control(ControlOp::And).as_str(), "and");
        assert_eq!(
            OperatorType::from_str("and"),
            Ok(OperatorType::Control(ControlOp::And))
        );

        assert_eq!(OperatorType::from_str("unknown"), Err("unknown operator"));
    }
}
