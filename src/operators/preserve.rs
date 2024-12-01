use crate::operators::operator::Operator;
use crate::{Error, JsonLogic, JsonLogicResult};
use serde_json::Value;

#[derive(Default)]
pub struct PreserveOperator;

impl Operator for PreserveOperator {
    fn apply(&self, _logic: &JsonLogic, args: &Value, _data: &Value) -> JsonLogicResult {
        match args {
            Value::Object(obj) => Ok(Value::Object(obj.clone())),
            _ => Err(Error::InvalidArguments("preserve requires object argument".into()))
        }
    }

    // Optionally disable auto traversal since we want to preserve the raw value
    fn auto_traverse(&self) -> bool {
        false
    }
}