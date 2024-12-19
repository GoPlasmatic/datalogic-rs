use serde_json::Value;
use crate::Error;
use super::{Operator, Rule, ValueCoercion};

pub struct MapOperator;
pub struct FilterOperator;
pub struct ReduceOperator;
pub struct AllOperator;
pub struct NoneOperator;
pub struct SomeOperator;
pub struct MergeOperator;

impl Operator for MapOperator {
    fn apply(&self, args: &[Rule], data: &Value) -> Result<Value, Error> {
        match args {
            [array_rule, mapper] => {
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
            },
            _ => Err(Error::InvalidArguments("map requires 2 arguments".into()))
        }
    }
}

impl Operator for FilterOperator {
    fn apply(&self, args: &[Rule], data: &Value) -> Result<Value, Error> {
        match args {
            [array_rule, predicate] => {
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
            },
            _ => Err(Error::InvalidArguments("filter requires 2 arguments".into()))
        }
    }
}

impl Operator for ReduceOperator {
    fn apply(&self, args: &[Rule], data: &Value) -> Result<Value, Error> {
        match args {
            [array_rule, reducer_rule, initial_rule] => {
                match array_rule.apply(data)? {
                    // Fast path: empty array
                    Value::Array(arr) if arr.is_empty() => initial_rule.apply(data),
                    
                    // Fast path: single element
                    Value::Array(arr) if arr.len() == 1 => {
                        let mut item_data = Value::Object(serde_json::Map::with_capacity(2));
                        let accumulator = initial_rule.apply(data)?;
                        item_data["current"] = arr[0].clone();
                        item_data["accumulator"] = accumulator;
                        reducer_rule.apply(&item_data)
                    },
                    
                    // Fast path: small arrays (2-4 elements)
                    Value::Array(arr) if arr.len() <= 4 => {
                        let mut item_data = Value::Object(serde_json::Map::with_capacity(2));
                        let mut accumulator = initial_rule.apply(data)?;
                        
                        // Unrolled loop for small arrays
                        for item in arr {
                            item_data["current"] = item;
                            item_data["accumulator"] = accumulator;
                            accumulator = reducer_rule.apply(&item_data)?;
                        }
                        Ok(accumulator)
                    },
                    
                    // Regular path: larger arrays
                    Value::Array(arr) => {
                        let mut item_data = Value::Object(serde_json::Map::with_capacity(2));
                        let mut accumulator = initial_rule.apply(data)?;
                        
                        // Process in chunks of 4 for better cache utilization
                        for chunk in arr.chunks(4) {
                            for item in chunk {
                                item_data["current"] = item.clone();
                                item_data["accumulator"] = accumulator;
                                accumulator = reducer_rule.apply(&item_data)?;
                            }
                        }
                        Ok(accumulator)
                    },
                    _ => initial_rule.apply(data),
                }
            },
            _ => Err(Error::InvalidArguments("reduce requires 3 arguments".into()))
        }
    }
}

impl Operator for AllOperator {
    fn apply(&self, args: &[Rule], data: &Value) -> Result<Value, Error> {
        match args {
            [array_rule, predicate] => {
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
            },
            _ => Err(Error::InvalidArguments("all requires 2 arguments".into()))
        }
    }
}

impl Operator for NoneOperator {
    fn apply(&self, args: &[Rule], data: &Value) -> Result<Value, Error> {
        match args {
            [array_rule, predicate] => {
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
            },
            _ => Err(Error::InvalidArguments("none requires 2 arguments".into()))
        }
    }
}

impl Operator for SomeOperator {
    fn apply(&self, args: &[Rule], data: &Value) -> Result<Value, Error> {
        match args {
            [array_rule, predicate] => {
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
            },
            _ => Err(Error::InvalidArguments("some requires 2 arguments".into()))
        }
    }
}

impl Operator for MergeOperator {
    fn apply(&self, args: &[Rule], data: &Value) -> Result<Value, Error> {
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