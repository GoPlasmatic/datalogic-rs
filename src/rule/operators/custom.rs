use serde_json::Value;
use crate::Error;
use super::{Rule, StaticEvaluable};
use std::borrow::Cow;
use std::sync::Arc;

pub trait CustomOperator: Send + Sync {
    fn name(&self) -> &str;
    fn apply<'a>(&self, args: &[Value], context: &'a Value, root: &'a Value, path: &str) -> Result<Cow<'a, Value>, Error>;
}

pub type CustomOperatorBox = Arc<dyn CustomOperator>;

pub struct CustomOperatorWrapper;

impl StaticEvaluable for CustomOperatorWrapper {
    fn is_static(&self, rule: &Rule) -> bool {
        if let Rule::Custom(_, args) = rule {
            args.iter().all(|r| r.is_static())
        } else {
            false
        }
    }
}