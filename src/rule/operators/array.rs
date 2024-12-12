use serde_json::Value;
use crate::Error;
use super::{Operator, Rule};

pub struct MapOperator;
pub struct FilterOperator;
pub struct ReduceOperator;
pub struct AllOperator;
pub struct NoneOperator;
pub struct SomeOperator;
pub struct MergeOperator;

fn is_truthy(value: &Value) -> bool {
    match value {
        Value::Null => false,
        Value::Bool(b) => *b,
        Value::Number(n) => n.as_f64().unwrap_or(0.0) != 0.0,
        Value::String(s) => !s.is_empty(),
        Value::Array(a) => !a.is_empty(),
        Value::Object(o) => !o.is_empty(),
    }
}

impl Operator for MapOperator {
    fn apply(&self, args: &[Rule], data: &Value) -> Result<Value, Error> {
        if args.len() != 2 {
            return Err(Error::InvalidArguments("map requires 2 arguments".to_string()));
        }
        
        let array = args[0].apply(data)?;
        let array = match array {
            Value::Array(arr) => arr,
            _ => return Ok(Value::Array(vec![])),
        };
        
        let mut results = Vec::new();
        for item in array {
            // Pass the array item as the data context for mapping
            let result = args[1].apply(&item)?;
            results.push(result);
        }
        
        Ok(Value::Array(results))
    }
}

impl Operator for FilterOperator {
    fn apply(&self, args: &[Rule], data: &Value) -> Result<Value, Error> {
        if args.len() != 2 {
            return Err(Error::InvalidArguments("filter requires 2 arguments".to_string()));
        }
    
        let array = args[0].apply(data)?;
        let array = match array {
            Value::Array(arr) => arr,
            _ => return Ok(Value::Array(vec![])),
        };
        
        let mut results = Vec::new();
        for item in array {
            // Pass item directly as context
            let result = args[1].apply(&item)?;
            if is_truthy(&result) {
                results.push(item);
            }
        }
        
        Ok(Value::Array(results))
    }
}


impl Operator for ReduceOperator {
    fn apply(&self, args: &[Rule], data: &Value) -> Result<Value, Error> {
        if args.len() != 3 {
            return Err(Error::InvalidArguments("reduce requires 3 arguments".to_string()));
        }
        
        let array = args[0].apply(data)?;
        let array = match array {
            Value::Array(arr) => arr,
            _ => return args[2].apply(data), // Return initial value instead of null
        };
        
        let mut accumulator = args[2].apply(data)?;
        for item in array {
            let mut item_data = serde_json::json!({
                "current": item,
                "accumulator": accumulator,
            });
            if let Value::Object(obj) = data {
                if let Value::Object(item_obj) = &mut item_data {
                    for (k, v) in obj {
                        item_obj.insert(k.clone(), v.clone());
                    }
                }
            }
            accumulator = args[1].apply(&item_data)?;
        }
        Ok(accumulator)
    }
}

impl Operator for AllOperator {
    fn apply(&self, args: &[Rule], data: &Value) -> Result<Value, Error> {
        if args.len() != 2 {
            return Err(Error::InvalidArguments("all requires 2 arguments".to_string()));
        }
        
        let array = args[0].apply(data)?;
        let array = match array {
            Value::Array(arr) => arr,
            _ => return Ok(Value::Bool(false)),
        };
        
        if array.is_empty() {
            return Ok(Value::Bool(false));
        }
    
        for item in array {
            // Pass item directly as context, similar to FilterOperator
            let result = args[1].apply(&item)?;
            if !is_truthy(&result) {
                return Ok(Value::Bool(false));
            }
        }
        
        Ok(Value::Bool(true))
    }
}

impl Operator for NoneOperator {
    fn apply(&self, args: &[Rule], data: &Value) -> Result<Value, Error> {
        if args.len() != 2 {
            return Err(Error::InvalidArguments("none requires 2 arguments".to_string()));
        }
        
        let array = args[0].apply(data)?;
        let array = match array {
            Value::Array(arr) => arr,
            _ => return Err(Error::InvalidRule("First argument must be array".to_string())),
        };
        
        for item in array {
            if args[1].apply(&item)?.as_bool().unwrap_or(false) {
                return Ok(Value::Bool(false));
            }
        }
        Ok(Value::Bool(true))
    }
}

impl Operator for SomeOperator {
    fn apply(&self, args: &[Rule], data: &Value) -> Result<Value, Error> {
        if args.len() != 2 {
            return Err(Error::InvalidArguments("some requires 2 arguments".to_string()));
        }
        
        let array = args[0].apply(data)?;
        let array = match array {
            Value::Array(arr) => arr,
            _ => return Err(Error::InvalidRule("First argument must be array".to_string())),
        };
        
        for item in array {
            if args[1].apply(&item)?.as_bool().unwrap_or(false) {
                return Ok(Value::Bool(true));
            }
        }
        Ok(Value::Bool(false))
    }
}

impl Operator for MergeOperator {
    fn apply(&self, args: &[Rule], data: &Value) -> Result<Value, Error> {
        let mut merged = Vec::new();
        
        for arg in args {
            let value = arg.apply(data)?;
            match value {
                Value::Array(arr) => merged.extend(arr),
                value => merged.push(value),
            }
        }
        
        Ok(Value::Array(merged))
    }
}