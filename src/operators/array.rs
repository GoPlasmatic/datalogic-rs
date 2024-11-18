use crate::operators::operator::Operator;
use crate::{JsonLogic, JsonLogicResult};
use serde_json::Value;

pub struct MergeOperator;

impl Operator for MergeOperator {
    fn apply(&self, logic: &JsonLogic, args: &Value, data: &Value) -> JsonLogicResult {
        let mut result = Vec::new();

        match args {
            Value::Array(values) => {
                for value in values {
                    let evaluated = logic.apply(value, data)?;
                    match evaluated {
                        Value::Array(arr) => result.extend(arr),
                        other => result.push(other),
                    }
                }
            },
            // Non-array arguments are converted to single-element arrays
            other => {
                let evaluated = logic.apply(other, data)?;
                result.push(evaluated);
            }
        }

        Ok(Value::Array(result))
    }
}