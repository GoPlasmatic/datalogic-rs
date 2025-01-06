use serde_json::Value;
use crate::Error;
use super::Rule;

pub struct PreserveOperator;

impl PreserveOperator {
    pub fn apply(&self, args: &[Rule], data: &Value) -> Result<Value, Error> {
        if args.len() != 1 {
            return Err(Error::InvalidArguments("preserve requires 1 argument".to_string()));
        }
        
        // Simply evaluate and return the value without any transformation
        args[0].apply(data)
    }
}