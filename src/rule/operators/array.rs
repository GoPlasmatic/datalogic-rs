use serde_json::Value;
use crate::JsonLogicResult;
use super::{Rule, ValueCoercion, ValueConvert, ArithmeticType};

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
        match array_rule.apply(data)? {
            Value::Array(arr) if arr.is_empty() => initial_rule.apply(data),
            Value::Array(arr) => {
                if let Rule::Arithmetic(op_type, _) = reducer_rule {
                    let init = initial_rule.apply(data)?.coerce_to_number();
                    
                    let result = match op_type {
                        ArithmeticType::Add => {
                            arr.iter()
                                .map(|v| v.coerce_to_number())
                                .fold(init, |acc, x| acc + x)
                        },
                        ArithmeticType::Multiply => {
                            arr.iter()
                                .map(|v| v.coerce_to_number())
                                .fold(init, |acc, x| acc * x)
                        },
                        _ => {
                            return self.standard_reduce(&arr, reducer_rule, initial_rule, data);
                        }
                    };
                    
                    return Ok(result.to_value());
                }

                self.standard_reduce(&arr, reducer_rule, initial_rule, data)
            },
            _ => initial_rule.apply(data),
        }
    }

    // Extract existing reduction logic to separate method
    fn standard_reduce(&self, arr: &[Value], reducer_rule: &Rule, initial_rule: &Rule, data: &Value) -> JsonLogicResult {
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
    pub fn apply(&self, array_rule: &Rule, predicate: &Rule, data: &Value, op_type: &ArrayPredicateType) -> JsonLogicResult {
        let result = if let Rule::Array(arr) = array_rule {
            if arr.is_empty() {
                match op_type {
                    ArrayPredicateType::All => true,
                    ArrayPredicateType::Some => false,
                    ArrayPredicateType::None => true,
                }
            } else {
                let predicate_fn = |rule: &Rule| -> bool {
                    let item = rule.apply(data).unwrap_or(Value::Null);
                    predicate.apply(&item)
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