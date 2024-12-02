mod error;
mod operators;

use error::Error;
use operators::preserve::PreserveOperator;
use operators::{
    operator::Operator, 
    var::VarOperator, 
    comparison::*, 
    logic::*, 
    arithmetic::*,
    string::*,
    array::*,
    missing::*,
};
use serde_json::Value;
use std::collections::HashMap;
use std::sync::Arc;

pub type JsonLogicResult = Result<Value, Error>;

#[derive(Clone)]
pub struct JsonLogic {
    var_op: Arc<VarOperator>,

    // Common comparison operators
    eq_op: Arc<EqualsOperator>,
    strict_eq_op: Arc<StrictEqualsOperator>,
    gt_op: Arc<GreaterThanOperator>,
    lt_op: Arc<LessThanOperator>,
    
    // Common logical operators
    and_op: Arc<AndOperator>,
    or_op: Arc<OrOperator>,
    not_op: Arc<NotOperator>,
    
    // Common array operators
    map_op: Arc<MapOperator>,
    filter_op: Arc<FilterOperator>,
    reduce_op: Arc<ReduceOperator>,

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
            var_op: Arc::new(VarOperator),
            eq_op: Arc::new(EqualsOperator),
            strict_eq_op: Arc::new(StrictEqualsOperator),
            gt_op: Arc::new(GreaterThanOperator),
            lt_op: Arc::new(LessThanOperator),
            and_op: Arc::new(AndOperator),
            or_op: Arc::new(OrOperator),
            not_op: Arc::new(NotOperator),
            map_op: Arc::new(MapOperator),
            filter_op: Arc::new(FilterOperator),
            reduce_op: Arc::new(ReduceOperator),
            operators: HashMap::new(),
        };
        logic.register_defaults();
        logic
    }

    fn register_defaults(&mut self) {
        self.operators.insert("!=".into(), Arc::new(NotEqualsOperator));
        self.operators.insert("!==".into(), Arc::new(StrictNotEqualsOperator));
        self.operators.insert(">=".into(), Arc::new(GreaterThanEqualOperator));
        self.operators.insert("<=".into(), Arc::new(LessThanEqualOperator));

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

        self.operators.insert("all".into(), Arc::new(AllOperator));
        self.operators.insert("none".into(), Arc::new(NoneOperator)); 
        self.operators.insert("some".into(), Arc::new(SomeOperator));
 
        self.operators.insert("preserve".into(), Arc::new(PreserveOperator));
    
    }

    pub fn apply(&self, logic: &Value, data: &Value) -> JsonLogicResult {
        match logic {
            Value::Object(map) if map.len() == 1 => {
                let (op, args) = map.iter().next().unwrap();
                let operator: &dyn Operator = match op.as_str() {
                    "var" => &*self.var_op,
                    "==" => &*self.eq_op,
                    "===" => &*self.strict_eq_op,
                    ">" => &*self.gt_op,
                    "<" => &*self.lt_op,
                    "and" => &*self.and_op,
                    "or" => &*self.or_op,
                    "!" => &*self.not_op,
                    "map" => &*self.map_op,
                    "filter" => &*self.filter_op,
                    "reduce" => &*self.reduce_op,
                    _ => &**self.operators.get(op)
                        .ok_or(Error::UnknownOperator(op.clone()))?
                };

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
                let mut results = Vec::with_capacity(values.len());
                for v in values {
                    results.push(self.apply(v, data)?);
                }
                Ok(Value::Array(results))
            }
            Value::String(_) | Value::Number(_) | Value::Bool(_) | Value::Null => {
                Ok(logic.clone())
            }
            _ => Err(Error::InvalidRule("Invalid Rule".to_string())),
        }
    }
}
