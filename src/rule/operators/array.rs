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

#[inline]
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
            return Err(Error::InvalidArguments("map requires 2 arguments".into()));
        }
        
        let array = match args[0].apply(data)? {
            Value::Array(arr) => arr,
            _ => return Ok(Value::Array(Vec::new())),
        };
        
        let results = array
            .into_iter()
            .map(|item| args[1].apply(&item))
            .collect::<Result<Vec<_>, _>>()?;
        
        Ok(Value::Array(results))
    }
}

impl Operator for FilterOperator {
    fn apply(&self, args: &[Rule], data: &Value) -> Result<Value, Error> {
        if args.len() != 2 {
            return Err(Error::InvalidArguments("filter requires 2 arguments".into()));
        }
    
        let array = match args[0].apply(data)? {
            Value::Array(arr) => arr,
            _ => return Ok(Value::Array(Vec::new())),
        };
        
        let results = array
            .into_iter()
            .filter(|item| matches!(args[1].apply(item), Ok(v) if is_truthy(&v)))
            .collect::<Vec<_>>();
        
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
            _ => return args[2].apply(data),
        };
        
        let mut item_data = Value::Object(serde_json::Map::with_capacity(2));
        let mut accumulator = args[2].apply(data)?;
        for item in array {
            item_data["current"] = item;
            item_data["accumulator"] = accumulator;
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
    
        let result = array
            .into_iter()
            .all(|item| matches!(args[1].apply(&item), Ok(v) if is_truthy(&v)));
            
        Ok(Value::Bool(result))
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
        
        let result = array
            .into_iter()
            .any(|item| matches!(args[1].apply(&item), Ok(v) if is_truthy(&v)));
            
        Ok(Value::Bool(result))
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