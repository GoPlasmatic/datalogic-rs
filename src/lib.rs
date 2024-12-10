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

    // Common arithmetic operators
    not_eq_op: Arc<NotEqualsOperator>,
    strict_not_eq_op: Arc<StrictNotEqualsOperator>,
    gt_eq_op: Arc<GreaterThanEqualOperator>,
    lt_eq_op: Arc<LessThanEqualOperator>,

    // Logic operators
    ternary_op: Arc<TernaryOperator>,
    double_bang_op: Arc<DoubleBangOperator>,
    if_op: Arc<IfOperator>,
    merge_op: Arc<MergeOperator>,
    missing_op: Arc<MissingOperator>,
    missing_some_op: Arc<MissingSomeOperator>,
    all_op: Arc<AllOperator>,
    none_op: Arc<NoneOperator>,
    some_op: Arc<SomeOperator>,

    preserve_op: Arc<PreserveOperator>,

    // String operators
    in_op: Arc<InOperator>,
    cat_op: Arc<CatOperator>,
    substr_op: Arc<SubstrOperator>,

    // Arithmetic operators
    add_op: Arc<AddOperator>,
    multiply_op: Arc<MultiplyOperator>,
    subtract_op: Arc<SubtractOperator>,
    divide_op: Arc<DivideOperator>,
    modulo_op: Arc<ModuloOperator>,
    max_op: Arc<MaxOperator>,
    min_op: Arc<MinOperator>,
}

impl Default for JsonLogic {
    fn default() -> Self {
        Self::new()
    }
}

impl JsonLogic {
    pub fn new() -> Self {
        Self {
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
            not_eq_op: Arc::new(NotEqualsOperator),
            strict_not_eq_op: Arc::new(StrictNotEqualsOperator),
            gt_eq_op: Arc::new(GreaterThanEqualOperator),
            lt_eq_op: Arc::new(LessThanEqualOperator),
            ternary_op: Arc::new(TernaryOperator),
            double_bang_op: Arc::new(DoubleBangOperator),
            if_op: Arc::new(IfOperator),
            merge_op: Arc::new(MergeOperator),
            missing_op: Arc::new(MissingOperator),
            missing_some_op: Arc::new(MissingSomeOperator),
            all_op: Arc::new(AllOperator),
            none_op: Arc::new(NoneOperator),
            some_op: Arc::new(SomeOperator),
            preserve_op: Arc::new(PreserveOperator),
            in_op: Arc::new(InOperator),
            cat_op: Arc::new(CatOperator),
            substr_op: Arc::new(SubstrOperator),
            add_op: Arc::new(AddOperator),
            multiply_op: Arc::new(MultiplyOperator),
            subtract_op: Arc::new(SubtractOperator),
            divide_op: Arc::new(DivideOperator),
            modulo_op: Arc::new(ModuloOperator),
            max_op: Arc::new(MaxOperator),
            min_op: Arc::new(MinOperator),
        }
    }

    fn get_operator(&self, op: &str) -> Result<&dyn Operator, Error> {
        match op {
            "var" => Ok(&*self.var_op),
            "==" => Ok(&*self.eq_op),
            "===" => Ok(&*self.strict_eq_op),
            ">" => Ok(&*self.gt_op),
            "<" => Ok(&*self.lt_op),
            "and" => Ok(&*self.and_op),
            "or" => Ok(&*self.or_op),
            "!" => Ok(&*self.not_op),
            "map" => Ok(&*self.map_op),
            "filter" => Ok(&*self.filter_op),
            "reduce" => Ok(&*self.reduce_op),
            "!=" => Ok(&*self.not_eq_op),
            "!==" => Ok(&*self.strict_not_eq_op),
            ">=" => Ok(&*self.gt_eq_op),
            "<=" => Ok(&*self.lt_eq_op),
            "?:" => Ok(&*self.ternary_op),
            "!!" => Ok(&*self.double_bang_op),
            "if" => Ok(&*self.if_op),
            "merge" => Ok(&*self.merge_op),
            "missing" => Ok(&*self.missing_op),
            "missing_some" => Ok(&*self.missing_some_op),
            "all" => Ok(&*self.all_op),
            "none" => Ok(&*self.none_op),
            "some" => Ok(&*self.some_op),
            "preserve" => Ok(&*self.preserve_op),
            "in" => Ok(&*self.in_op),
            "cat" => Ok(&*self.cat_op),
            "substr" => Ok(&*self.substr_op),
            "+" => Ok(&*self.add_op),
            "*" => Ok(&*self.multiply_op),
            "-" => Ok(&*self.subtract_op),
            "/" => Ok(&*self.divide_op),
            "%" => Ok(&*self.modulo_op),
            "max" => Ok(&*self.max_op),
            "min" => Ok(&*self.min_op),
            _ => Err(Error::UnknownOperator(op.to_string())),
        }
    }

    pub fn apply(&self, logic: &Value, data: &Value) -> JsonLogicResult {
        match logic {
            Value::String(_) | Value::Number(_) | Value::Bool(_) | Value::Null => {
                Ok(logic.clone())
            }
            Value::Object(map) if map.len() == 1 => {
                let (op, args) = map.iter().next().unwrap();
                let operator: &dyn Operator = self.get_operator(op)?;

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
            _ => Err(Error::InvalidRule("Invalid Rule".to_string())),
        }
    }
}
