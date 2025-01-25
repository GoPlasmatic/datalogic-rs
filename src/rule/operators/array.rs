use serde_json::Value;
use crate::JsonLogicResult;
use super::{Rule, ValueCoercion};

pub struct MapOperator;
pub struct FilterOperator;
pub struct ReduceOperator;
pub struct MergeOperator;

impl MapOperator {
    pub fn apply(&self, array_rule: &Rule, mapper: &Rule, data: &Value) -> JsonLogicResult {
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
        match array_rule.apply(data)? {
            Value::Array(arr) => {
                let results = arr
                    .into_iter()
                    .filter(|item| matches!(predicate.apply(item), Ok(v) if v.coerce_to_bool()))
                    .collect::<Vec<_>>();
                
                Ok(Value::Array(results))
            },
            _ => Ok(Value::Array(Vec::new()))
        }
    }
}

impl ReduceOperator {
    pub fn apply(&self, array_rule: &Rule, reducer_rule: &Rule, initial_rule: &Rule, data: &Value) -> JsonLogicResult {
        static CURRENT: &str = "current";
        static ACCUMULATOR: &str = "accumulator";

        match array_rule.apply(data)? {
            Value::Array(arr) if arr.is_empty() => initial_rule.apply(data),
            Value::Array(arr) => {
                let mut map = serde_json::Map::with_capacity(2);
                map.insert(CURRENT.to_string(), Value::Null);
                map.insert(ACCUMULATOR.to_string(), initial_rule.apply(data)?);
                let mut item_data = Value::Object(map);

                for item in arr {
                    if let Value::Object(ref mut map) = item_data {
                        map.insert(CURRENT.to_string(), item);
                    }

                    let result = reducer_rule.apply(&item_data)?;

                    if let Value::Object(ref mut map) = item_data {
                        map.insert(ACCUMULATOR.to_string(), result);
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
    None
}

pub struct ArrayPredicateOperator;

impl ArrayPredicateOperator {
    pub fn apply(&self, array_rule: &Rule, predicate: &Rule, data: &Value, op_type: ArrayPredicateType) -> JsonLogicResult {
        let array_value = match array_rule {
            Rule::Value(value) => value,
            _ => &array_rule.apply(data)?
        };

        let result = if let Value::Array(arr) = array_value {
            if arr.is_empty() {
                match op_type {
                    ArrayPredicateType::All => true,
                    ArrayPredicateType::Some => false,
                    ArrayPredicateType::None => true,
                }
            } else {
                let predicate_fn = |item: &Value| -> bool {
                    predicate.apply(item)
                        .map(|v| v.coerce_to_bool())
                        .unwrap_or(false)
                };

                match op_type {
                    ArrayPredicateType::All | ArrayPredicateType::Some => arr.iter().any(predicate_fn),
                    ArrayPredicateType::None => !arr.iter().any(predicate_fn),
                }
            }
        } else {
            false
        };

        Ok(Value::Bool(result))
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