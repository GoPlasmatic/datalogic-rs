use serde_json::Value;
use super::{Rule, Error, ValueExt};
use std::borrow::Cow;

#[derive(Debug, Clone)]
pub enum CompareType { Equals, StrictEquals, NotEquals, StrictNotEquals, GreaterThan, LessThan, GreaterThanEqual, LessThanEqual }

pub struct CompareOperator;

impl CompareOperator {
    pub fn apply<'a>(&self, args: &[Rule], data: &'a Value, compare_type: &CompareType) -> Result<Cow<'a, Value>, Error> {
        match args.len() {
            0 | 1 => Err(Error::Custom("Invalid Arguments".to_string())),
            2 => {
                let left = args[0].apply(data)?;
                let right = args[1].apply(data)?;
                Ok(Cow::Owned(Value::Bool(self.compare(&left, &right, compare_type)?)))
            }
            3 => {
                let first = args[0].apply(data)?;
                let second = args[1].apply(data)?;
                if !self.compare(&first, &second, compare_type)? {
                    return Ok(Cow::Owned(Value::Bool(false)));
                }
                let third = args[2].apply(data)?;
                Ok(Cow::Owned(Value::Bool(self.compare(&second, &third, compare_type)?)))
            }
            _ => {
                let mut prev = args[0].apply(data)?;
                for arg in args.iter().skip(1) {
                    let curr = arg.apply(data)?;
                    if !self.compare(&prev, &curr, compare_type)? {
                        return Ok(Cow::Owned(Value::Bool(false)));
                    }
                    prev = curr;
                }
                Ok(Cow::Owned(Value::Bool(true)))
            }
        }
    }

    fn compare(&self, left: &Value, right: &Value, compare_type: &CompareType) -> Result<bool, Error> {
        use CompareType::*;

        match compare_type {
            StrictEquals => left.strict_equals(right),
            StrictNotEquals => left.strict_not_equals(right),
            Equals => left.equals(right),
            NotEquals => left.not_equals(right),
            GreaterThan => left.greater_than(right),
            GreaterThanEqual => left.greater_than_equal(right),
            LessThan => left.less_than(right),
            LessThanEqual => left.less_than_equal(right),
        }
    }
}