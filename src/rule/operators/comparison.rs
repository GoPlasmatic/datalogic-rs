use serde_json::Value;
use crate::JsonLogicResult;
use super::{Rule, ValueCoercion, Error};


#[derive(Debug, Clone)]
pub enum CompareType { Equals, StrictEquals, NotEquals, StrictNotEquals, GreaterThan, LessThan, GreaterThanEqual, LessThanEqual }

pub struct CompareOperator;

impl CompareOperator {
    pub fn apply(&self, args: &[Rule], data: &Value, compare_type: &CompareType) -> JsonLogicResult {
        match args {
            [a, b] => {
                let left = a.apply(data)?;
                let right = b.apply(data)?;
                
                Ok(Value::Bool(self.compare(&left, &right, compare_type)?))
            },
            args if args.len() > 2 => {
                let mut prev = args[0].apply(data)?;
                for arg in args.iter().skip(1) {
                    let curr = arg.apply(data)?;
                    if !self.compare(&prev, &curr, compare_type)? {
                        return Ok(Value::Bool(false));
                    }
                    prev = curr;
                }
                Ok(Value::Bool(true))
            }
            _ => Err(Error::Custom("Invalid Arguments".to_string()))
        }
    }

    #[inline]
    fn compare(&self, left: &Value, right: &Value, compare_type: &CompareType) -> Result<bool, Error> {
        use CompareType::*;
        
        match compare_type {
            StrictEquals => {
                return Ok(std::mem::discriminant(left) == std::mem::discriminant(right) && left == right);
            }
            StrictNotEquals => {
                return Ok(std::mem::discriminant(left) != std::mem::discriminant(right) || left != right);
            }
            _ => {}
        }
    
        if let (Value::String(s1), Value::String(s2)) = (left, right) {
            return Ok(match compare_type {
                GreaterThan => s1 > s2,
                LessThan => s1 < s2,
                GreaterThanEqual => s1 >= s2,
                LessThanEqual => s1 <= s2,
                Equals => s1 == s2,
                NotEquals => s1 != s2,
                _ => unreachable!()
            });
        }
        
        if matches!(compare_type, GreaterThan | LessThan | GreaterThanEqual | LessThanEqual) {
            let l_num = left.coerce_to_number()?;
            let r_num = right.coerce_to_number()?;
            return Ok(match compare_type {
                GreaterThan => l_num > r_num,
                LessThan => l_num < r_num,
                GreaterThanEqual => l_num >= r_num,
                LessThanEqual => l_num <= r_num,
                _ => unreachable!()
            });
        }

        match compare_type {
            Equals | NotEquals => {
                if let (Value::Number(n1), Value::Number(n2)) = (left, right) {
                    return Ok(match compare_type {
                        Equals => n1 == n2,
                        NotEquals => n1 != n2,
                        _ => unreachable!()
                    });
                }

                let l_num = left.coerce_to_number()?;
                let r_num = right.coerce_to_number()?;
                Ok(match compare_type {
                    Equals => l_num == r_num,
                    NotEquals => l_num != r_num,
                    _ => unreachable!()
                })
            }
            _ => unreachable!()
        }
    }
}