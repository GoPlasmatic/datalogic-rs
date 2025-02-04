use std::str::FromStr;

use thiserror::Error;

#[derive(Debug, PartialEq, Clone)]
pub enum OpType {
    Var,
    // Comparison
    Equals, StrictEquals, NotEquals, StrictNotEquals,
    GreaterThan, LessThan, GreaterThanEqual, LessThanEqual,
    // Arithmetic
    Add, Multiply, Subtract, Divide, Modulo, Max, Min,
    // Logic
    And, Or, Not, DoubleBang,
    // Control
    If, Ternary,
    // String
    In, Cat, Substr,
    // Array
    Map, Filter, Reduce, Merge,
    All, Some, None,
    // Missing
    Missing, MissingSome,
    // Invalid
    Invalid,
}

#[derive(Debug, Error)]
#[error("Invalid operation type: {0}")]
pub struct ParseOpTypeError(String);

impl FromStr for OpType {
    type Err = ParseOpTypeError;

    fn from_str(op: &str) -> Result<Self, Self::Err> {
        match op {
            // Variable access
            "var" => Ok(Self::Var),
            
            // Comparison operators
            "==" => Ok(Self::Equals),
            "!=" => Ok(Self::NotEquals),
            "===" => Ok(Self::StrictEquals),
            "!==" => Ok(Self::StrictNotEquals),
            ">" => Ok(Self::GreaterThan),
            "<" => Ok(Self::LessThan),
            ">=" => Ok(Self::GreaterThanEqual),
            "<=" => Ok(Self::LessThanEqual),
            
            // Arithmetic operators
            "+" => Ok(Self::Add),
            "*" => Ok(Self::Multiply),
            "-" => Ok(Self::Subtract),
            "/" => Ok(Self::Divide),
            "%" => Ok(Self::Modulo),
            "max" => Ok(Self::Max),
            "min" => Ok(Self::Min),
            
            // Logic operators
            "and" => Ok(Self::And),
            "or" => Ok(Self::Or),
            "!" => Ok(Self::Not),
            "!!" => Ok(Self::DoubleBang),
            
            // Control flow
            "if" => Ok(Self::If),
            "?:" => Ok(Self::Ternary),
            
            // String operations
            "in" => Ok(Self::In),
            "cat" => Ok(Self::Cat),
            "substr" => Ok(Self::Substr),
            
            // Array operations
            "map" => Ok(Self::Map),
            "filter" => Ok(Self::Filter),
            "reduce" => Ok(Self::Reduce),
            "merge" => Ok(Self::Merge),
            "all" => Ok(Self::All),
            "some" => Ok(Self::Some),
            "none" => Ok(Self::None),
            
            // Missing checks
            "missing" => Ok(Self::Missing),
            "missing_some" => Ok(Self::MissingSome),
            
            // Invalid operation
            _ => Err(ParseOpTypeError(op.to_string())),
        }
    }
}
