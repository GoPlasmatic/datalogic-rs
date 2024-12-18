mod error;
mod rule;

use error::Error;
use serde_json::Value;
pub use rule::Rule;

pub type JsonLogicResult = Result<Value, Error>;

#[derive(Clone)]
pub struct JsonLogic {
}

impl Default for JsonLogic {
    fn default() -> Self {
        Self::new()
    }
}

impl JsonLogic {
    pub fn new() -> Self {
        Self {}
    }

    pub fn apply(rule: &Rule, data: &Value) -> JsonLogicResult {
        rule.apply(data)
    }
}
