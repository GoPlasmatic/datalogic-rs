use serde_json::Value;
use crate::{JsonLogicResult, Error};
use super::{Rule, ValueCoercion};

pub struct MapOperator;
pub struct FilterOperator;
pub struct ReduceOperator;
pub struct MergeOperator;

impl MapOperator {
    pub fn apply(&self, array_rule: &Rule, mapper: &Rule, data: &Value) -> JsonLogicResult {
        if let Rule::Value(arr_val) = array_rule {
            if let Rule::Value(mapper_val) = mapper {
                if arr_val.is_null() && mapper_val.is_null() {
                    return Err(Error::CustomError("Invalid Arguments".into()));
                }
            }
        }

        match array_rule.apply(data)? {
            Value::Array(arr) => {
                let results = arr
                    .into_iter()
                    .map(|item| mapper.apply(&item))
                    .collect::<Result<Vec<_>, _>>()?;
                
                Ok(Value::Array(results))
            },
            _ => Ok(Value::Array(Vec::new()))
        }
    }
}

impl FilterOperator {
    pub fn apply(&self, array_rule: &Rule, predicate: &Rule, data: &Value) -> JsonLogicResult {
        if let Rule::Value(arr_val) = array_rule {
            if let Rule::Value(predicate_val) = predicate {
                if arr_val.is_null() && predicate_val.is_null() {
                    return Err(Error::CustomError("Invalid Arguments".into()));
                }
            }
        }

        match array_rule.apply(data)? {
            Value::Array(arr) => {
                let results = arr
                    .into_iter()
                    .filter(|item| matches!(predicate.apply(item), Ok(v) if v.coerce_to_bool()))
                    .collect::<Vec<_>>();
                
                Ok(Value::Array(results))
            },
            _ => Err(Error::CustomError("Invalid Arguments".into()))
        }
    }
}

impl ReduceOperator {
    pub fn apply(&self, array_rule: &Rule, reducer_rule: &Rule, initial_rule: &Rule, data: &Value) -> JsonLogicResult {
        match array_rule.apply(data)? {
            Value::Array(arr) if arr.is_empty() => initial_rule.apply(data),
            Value::Array(arr) => {
                static CURRENT: &str = "current";
                static ACCUMULATOR: &str = "accumulator";
        
                let mut map = serde_json::Map::with_capacity(2);
                map.insert(CURRENT.to_string(), Value::Null);
                map.insert(ACCUMULATOR.to_string(), initial_rule.apply(data)?);
                let mut item_data = Value::Object(map);
        
                for item in arr {
                    if let Value::Object(ref mut map) = item_data {
                        map[&CURRENT.to_string()] = item.clone();
                    }
        
                    let result = reducer_rule.apply(&item_data)?;
        
                    if let Value::Object(ref mut map) = item_data {
                        map[&ACCUMULATOR.to_string()] = result;
                    }
                }
        
                match item_data {
                    Value::Object(map) => Ok(map.get(ACCUMULATOR).cloned().unwrap_or(Value::Null)),
                    _ => Ok(Value::Null)
                }
            },
            _ => initial_rule.apply(data),
        }
    }
}

#[derive(Debug, Clone)]
pub enum ArrayPredicateType {
    All,
    Some,
    None,
    Invalid
}

pub struct ArrayPredicateOperator;

impl ArrayPredicateOperator {
    pub fn apply(&self, array_rule: &Rule, predicate: &Rule, data: &Value, op_type: &ArrayPredicateType) -> JsonLogicResult {
        let array_value = array_rule.apply(data)?;
        
        match array_value {
            Value::Array(arr) => {
                match op_type {
                    ArrayPredicateType::All => {
                        if arr.is_empty() {
                            return Ok(Value::Bool(false));
                        }
                        let result = arr.iter()
                            .all(|item| matches!(predicate.apply(item), Ok(v) if v.coerce_to_bool()));
                        Ok(Value::Bool(result))
                    },
                    ArrayPredicateType::Some => {
                        if arr.is_empty() {
                            return Ok(Value::Bool(false));
                        }
                        let result = arr.iter()
                            .any(|item| matches!(predicate.apply(item), Ok(v) if v.coerce_to_bool()));
                        Ok(Value::Bool(result))
                    },
                    ArrayPredicateType::None => {
                        if arr.is_empty() {
                            return Ok(Value::Bool(true));
                        }
                        let result = arr.iter()
                            .any(|item| matches!(predicate.apply(item), Ok(v) if v.coerce_to_bool()));
                        Ok(Value::Bool(!result))
                    },
                    _ => Err(Error::CustomError("Invalid Arguments".into()))
                }
            },
            _ => Err(Error::CustomError("Invalid Arguments".into()))
        }
    }
}

impl MergeOperator {
    pub fn apply(&self, args: &[Rule], data: &Value) -> JsonLogicResult {
        if args.is_empty() {
            return Ok(Value::Array(Vec::new()));
        }
        
        let capacity = args.len() * 2;
        let mut merged = Vec::with_capacity(capacity);
        
        for arg in args {
            match arg.apply(data)? {
                Value::Array(arr) => merged.extend(arr),
                value => merged.push(value),
            }
        }
        
        Ok(Value::Array(merged))
    }
}