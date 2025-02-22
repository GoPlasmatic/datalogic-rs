use serde_json::Value;
use crate::Error;
use super::{Rule, ValueCoercion};
use std::borrow::Cow;

pub struct MapOperator;
pub struct FilterOperator;
pub struct ReduceOperator;
pub struct MergeOperator;

impl MapOperator {
    pub fn apply<'a>(&self, array_rule: &Rule, mapper: &Rule, data: &'a Value) -> Result<Cow<'a, Value>, Error> {
        if let Rule::Value(arr_val) = array_rule {
            if arr_val.is_null() {
                return Err(Error::Custom("Invalid Arguments".into()));
            }
        }
        if let Rule::Value(mapper_val) = mapper {
            if mapper_val.is_null() {
                return Err(Error::Custom("Invalid Arguments".into()));
            }
        }

        let array_value = array_rule.apply(data)?;
        match array_value.as_ref() {
            Value::Array(arr) => {
                let mut results = Vec::with_capacity(arr.len());
                for item in arr {
                    results.push(mapper.apply(item)?.into_owned());
                }
                Ok(Cow::Owned(Value::Array(results)))
            },
            _ => Ok(Cow::Owned(Value::Array(Vec::new())))
        }
    }
}

impl FilterOperator {
    pub fn apply<'a>(&self, array_rule: &Rule, predicate: &Rule, data: &'a Value) -> Result<Cow<'a, Value>, Error> {
        if let Rule::Value(arr_val) = array_rule {
            if arr_val.is_null() {
                return Err(Error::Custom("Invalid Arguments".into()));
            }
        }
        if let Rule::Value(predicate_val) = predicate {
            if predicate_val.is_null() {
                return Err(Error::Custom("Invalid Arguments".into()));
            }
        }

        let array_value = array_rule.apply(data)?;
        match array_value.as_ref() {
            Value::Array(arr) => {
                let results = arr
                    .iter()
                    .filter(|item| matches!(predicate.apply(item), Ok(cow) if cow.coerce_to_bool()))
                    .cloned()
                    .collect();
                
                Ok(Cow::Owned(Value::Array(results)))
            },
            _ => Ok(Cow::Owned(Value::Array(Vec::with_capacity(0))))
        }
    }
}

impl ReduceOperator {
    pub fn apply<'a>(&self, array_rule: &Rule, reducer_rule: &Rule, initial_rule: &'a Rule, data: &'a Value) -> Result<Cow<'a, Value>, Error> {
        let array_value = array_rule.apply(data)?;

        match array_value.as_ref() {
            Value::Array(arr) if arr.is_empty() => {
                initial_rule.apply(data)
            },
            Value::Array(arr) => {
                let mut accumulator = initial_rule.apply(data)?.into_owned();

                for current in arr {
                    let mut context = serde_json::Map::with_capacity(2);
                    context.insert("current".to_string(), current.to_owned());
                    context.insert("accumulator".to_string(), accumulator);
                    
                    accumulator = reducer_rule.apply(&Value::Object(context))?.into_owned();
                }
                
                Ok(Cow::Owned(accumulator))
            },
            _ => initial_rule.apply(data),
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum ArrayPredicateType {
    All,
    Some,
    None,
    Invalid
}

pub struct ArrayPredicateOperator;

impl ArrayPredicateOperator {
    pub fn apply<'a>(&self, array_rule: &Rule, predicate: &Rule, data: &'a Value, op_type: &ArrayPredicateType) 
        -> Result<Cow<'a, Value>, Error> 
    {
        if *op_type == ArrayPredicateType::Invalid {
            return Err(Error::Custom("Invalid Arguments".into()));
        }

        let array_value = array_rule.apply(data)?;

        match array_value.as_ref() {
            Value::Array(arr) => {
                let result = match op_type {
                    ArrayPredicateType::All => {
                        if arr.is_empty() {
                            false
                        } else {
                            arr.iter()
                                .all(|item| matches!(predicate.apply(item), Ok(v) if v.coerce_to_bool()))
                        }
                    },
                    ArrayPredicateType::Some => {
                        if arr.is_empty() {
                            false
                        } else {
                            arr.iter()
                                .any(|item| matches!(predicate.apply(item), Ok(v) if v.coerce_to_bool()))
                        }
                    },
                    ArrayPredicateType::None => {
                        if arr.is_empty() {
                            true
                        } else {
                            !arr.iter()
                                .any(|item| matches!(predicate.apply(item), Ok(v) if v.coerce_to_bool()))
                        }
                    },
                    _ => unreachable!()
                };
                Ok(Cow::Owned(Value::Bool(result)))
            },
            _ => Err(Error::Custom("Invalid Arguments".into()))
        }
    }
}

impl MergeOperator {
    pub fn apply<'a>(&self, args: &[Rule], data: &'a Value) -> Result<Cow<'a, Value>, Error> {
        if args.is_empty() {
            return Ok(Cow::Owned(Value::Array(Vec::new())));
        }
        
        let capacity = args.len() * 2;
        let mut merged = Vec::with_capacity(capacity);
        
        for arg in args {
            match arg.apply(data)? {
                Cow::Owned(Value::Array(arr)) => merged.extend(arr),
                Cow::Borrowed(Value::Array(arr)) => merged.extend(arr.iter().cloned()),
                value => merged.push(value.into_owned()),
            }
        }
        
        Ok(Cow::Owned(Value::Array(merged)))
    }
}