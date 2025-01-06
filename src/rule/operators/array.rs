use serde_json::Value;
use crate::Error;
use super::{Rule, ValueCoercion};

pub struct MapOperator;
pub struct FilterOperator;
pub struct ReduceOperator;
pub struct AllOperator;
pub struct NoneOperator;
pub struct SomeOperator;
pub struct MergeOperator;

impl MapOperator {
    pub fn apply(&self, array_rule: &Rule, mapper: &Rule, data: &Value) -> Result<Value, Error> {
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
    pub fn apply(&self, array_rule: &Rule, predicate: &Rule, data: &Value) -> Result<Value, Error> {
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
    pub fn apply(&self, array_rule: &Rule, reducer_rule: &Rule, initial_rule: &Rule, data: &Value) -> Result<Value, Error> {
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

impl AllOperator {
    pub fn apply(&self, array_rule: &Rule, predicate: &Rule, data: &Value) -> Result<Value, Error> {
        match array_rule.apply(data)? {
            Value::Array(arr) if arr.is_empty() => Ok(Value::Bool(false)),
            Value::Array(arr) => {
                let result = arr
                    .into_iter()
                    .all(|item| matches!(predicate.apply(&item), Ok(v) if v.coerce_to_bool()));
                
                Ok(Value::Bool(result))
            },
            _ => Ok(Value::Bool(false))
        }
    }
}

impl NoneOperator {
    pub fn apply(&self, array_rule: &Rule, predicate: &Rule, data: &Value) -> Result<Value, Error> {
        match array_rule.apply(data)? {
            Value::Array(arr) if arr.is_empty() => Ok(Value::Bool(true)),
            Value::Array(arr) => {
                let result = arr
                    .iter()
                    .any(|item| matches!(predicate.apply(item), Ok(v) if v.coerce_to_bool()));
                Ok(Value::Bool(!result))
            },
            _ => Err(Error::InvalidRule("First argument must be array".into()))
        }
    }
}

impl SomeOperator {
    pub fn apply(&self, array_rule: &Rule, predicate: &Rule, data: &Value) -> Result<Value, Error> {
        match array_rule.apply(data)? {
            Value::Array(arr) if arr.is_empty() => Ok(Value::Bool(false)),
            Value::Array(arr) => {
                let result = arr
                    .iter()
                    .any(|item| matches!(predicate.apply(item), Ok(v) if v.coerce_to_bool()));
                Ok(Value::Bool(result))
            },
            _ => Err(Error::InvalidRule("First argument must be array".into()))
        }
    }
}

impl MergeOperator {
    pub fn apply(&self, args: &[Rule], data: &Value) -> Result<Value, Error> {
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