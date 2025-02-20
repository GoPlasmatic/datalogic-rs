use serde_json::Value;
use crate::Error;
use std::borrow::Cow;
use std::sync::Arc;

pub trait CustomOperator: Send + Sync {
    fn name(&self) -> &str;
    fn apply<'a>(&self, args: &[Value], data: &'a Value) -> Result<Cow<'a, Value>, Error>;
}

pub type CustomOperatorBox = Arc<dyn CustomOperator>;