use serde_json::Value;
use crate::Error;
use std::borrow::Cow;
use std::sync::Arc;

pub trait CustomOperator: Send + Sync {
    fn name(&self) -> &str;
    fn apply<'a>(&self, args: &[Value], context: &'a Value, root: &'a Value, path: &str) -> Result<Cow<'a, Value>, Error>;
}

pub type CustomOperatorBox = Arc<dyn CustomOperator>;