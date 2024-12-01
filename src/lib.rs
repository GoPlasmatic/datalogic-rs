mod error;
mod operators;

use error::Error;
use operators::{
    operator::Operator, 
    var::VarOperator, 
    comparison::*, 
    logic::*, 
    arithmetic::*,
    string::*,
    array::*,
    missing::*,
    array_ops::*,
};
use serde_json::Value;
use std::collections::HashMap;
use std::sync::Arc;

pub type JsonLogicResult = Result<Value, Error>;

#[derive(Clone)]
pub struct JsonLogic {
    operators: HashMap<String, Arc<dyn Operator>>,
}

impl Default for JsonLogic {
    fn default() -> Self {
        Self::new()
    }
}

impl JsonLogic {
    pub fn new() -> Self {
        let mut logic = Self {
            operators: HashMap::new(),
        };
        logic.register_defaults();
        logic
    }

    fn register_defaults(&mut self) {
        self.operators.insert("var".into(), Arc::new(VarOperator));

        self.operators.insert("==".into(), Arc::new(EqualsOperator));
        self.operators.insert("===".into(), Arc::new(StrictEqualsOperator));
        self.operators.insert("!=".into(), Arc::new(NotEqualsOperator));
        self.operators.insert("!==".into(), Arc::new(StrictNotEqualsOperator));
        self.operators.insert(">".into(), Arc::new(GreaterThanOperator));
        self.operators.insert(">=".into(), Arc::new(GreaterThanEqualOperator));
        self.operators.insert("<".into(), Arc::new(LessThanOperator));
        self.operators.insert("<=".into(), Arc::new(LessThanEqualOperator));
        self.operators.insert("!".into(), Arc::new(NotOperator));

        self.operators.insert("or".into(), Arc::new(OrOperator));
        self.operators.insert("and".into(), Arc::new(AndOperator));
        self.operators.insert("?:".into(), Arc::new(TernaryOperator));
        self.operators.insert("!!".into(), Arc::new(DoubleBangOperator));

        self.operators.insert("in".into(), Arc::new(InOperator));
        self.operators.insert("cat".into(), Arc::new(CatOperator));
        self.operators.insert("substr".into(), Arc::new(SubstrOperator));

        self.operators.insert("+".into(), Arc::new(AddOperator));
        self.operators.insert("*".into(), Arc::new(MultiplyOperator));
        self.operators.insert("-".into(), Arc::new(SubtractOperator));
        self.operators.insert("/".into(), Arc::new(DivideOperator));
        self.operators.insert("%".into(), Arc::new(ModuloOperator));
        self.operators.insert("max".into(), Arc::new(MaxOperator));
        self.operators.insert("min".into(), Arc::new(MinOperator));

        self.operators.insert("merge".into(), Arc::new(MergeOperator));

        self.operators.insert("if".into(), Arc::new(IfOperator));

        self.operators.insert("missing".into(), Arc::new(MissingOperator));
        self.operators.insert("missing_some".into(), Arc::new(MissingSomeOperator));

        self.operators.insert("filter".into(), Arc::new(FilterOperator));
        self.operators.insert("map".into(), Arc::new(MapOperator));
        self.operators.insert("reduce".into(), Arc::new(ReduceOperator));
        self.operators.insert("all".into(), Arc::new(AllOperator));
        self.operators.insert("none".into(), Arc::new(NoneOperator)); 
        self.operators.insert("some".into(), Arc::new(SomeOperator));
 
    
    }

    pub fn apply(&self, logic: &Value, data: &Value) -> JsonLogicResult {
        match logic {
            Value::Object(map) if map.len() == 1 => {
                let (op, args) = map.iter().next().unwrap();
                let operator = self.operators
                    .get(op)
                    .ok_or(Error::UnknownOperator(op.clone()))?;

                // Handle automatic traversal
                if operator.auto_traverse() {
                    match args {
                        Value::Array(values) => {
                            let evaluated = values
                                .iter()
                                .map(|v| self.apply(v, data))
                                .collect::<Result<Vec<_>, _>>()?;
                            operator.apply(self, &Value::Array(evaluated), data)
                        }
                        _ => operator.apply(self, args, data)
                    }
                } else {
                    operator.apply(self, args, data)
                }
            }
            Value::Array(values) => {
                // Recursively evaluate each array element
                let results = values
                    .iter()
                    .map(|v| self.apply(v, data))
                    .collect::<Result<Vec<_>, _>>()?;
                Ok(Value::Array(results))
            }
            Value::String(_) | Value::Number(_) | Value::Bool(_) | Value::Null => {
                Ok(logic.clone())
            }
            _ => Err(Error::InvalidRule("Invalid Rule".to_string())),
        }
    }
}
