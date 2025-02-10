use serde_json::Value;
use crate::JsonLogicResult;
use super::{Rule, ValueCoercion};


#[derive(Debug, Clone)]
pub enum CompareType { Equals, StrictEquals, NotEquals, StrictNotEquals, GreaterThan, LessThan, GreaterThanEqual, LessThanEqual }

pub struct CompareOperator;

impl CompareOperator {
    pub fn apply(&self, args: &[Rule], data: &Value, compare_type: &CompareType) -> JsonLogicResult {
        match args {
            [a, b] => {
                let left = a.apply(data)?;
                let right = b.apply(data)?;
                
                Ok(Value::Bool(self.compare(&left, &right, &compare_type)))
            },
            args if args.len() > 2 => {
                let mut prev = args[0].apply(data)?;
                for arg in args.iter().skip(1) {
                    let curr = arg.apply(data)?;
                    if !self.compare(&prev, &curr, &compare_type) {
                        return Ok(Value::Bool(false));
                    }
                    prev = curr;
                }
                Ok(Value::Bool(true))
            }
            _ => Ok(Value::Bool(false))
        }
    }

    fn compare(&self, left: &Value, right: &Value, compare_type: &CompareType) -> bool {
        use CompareType::*;
        
        match compare_type {
            StrictEquals => {
                return std::mem::discriminant(left) == std::mem::discriminant(right) && left == right;
            }
            StrictNotEquals => {
                return std::mem::discriminant(left) != std::mem::discriminant(right) || left != right;
            }
            _ => {}
        }
    
        match compare_type {
            GreaterThan | LessThan | GreaterThanEqual | LessThanEqual => {
                let l_num = left.coerce_to_number();
                let r_num = right.coerce_to_number();
                return match compare_type {
                    GreaterThan => l_num > r_num,
                    LessThan => l_num < r_num,
                    GreaterThanEqual => l_num >= r_num,
                    LessThanEqual => l_num <= r_num,
                    _ => unreachable!()
                };
            }
            _ => {}
        }
    
        match (left, right) {
            (Value::Number(n1), Value::Number(n2)) => {
                match compare_type {
                    Equals => n1 == n2,
                    NotEquals => n1 != n2,
                    _ => unreachable!()
                }
            }
            (Value::String(s1), Value::String(s2)) => {
                match compare_type {
                    Equals => s1 == s2,
                    NotEquals => s1 != s2,
                    _ => unreachable!()
                }
            }
            (Value::Bool(b1), Value::Bool(b2)) => {
                match compare_type {
                    Equals => b1 == b2,
                    NotEquals => b1 != b2,
                    _ => unreachable!()
                }
            }
            _ => match compare_type {
                Equals => left.coerce_to_number() == right.coerce_to_number(),
                NotEquals => left.coerce_to_number() != right.coerce_to_number(),
                _ => unreachable!()
            }
        }
    }
}