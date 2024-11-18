use crate::{JsonLogic, JsonLogicResult};
use serde_json::Value;

pub trait Operator: Send + Sync {
    fn apply(&self, logic: &JsonLogic, args: &Value, data: &Value) -> JsonLogicResult;
}

