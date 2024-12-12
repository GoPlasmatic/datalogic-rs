mod operators;

use serde_json::Value;
use crate::Error;

pub use operators::*;

#[derive(Debug)]
pub enum Rule {
    // Variable access
    Var(Vec<Rule>),
    
    // Comparison operators
    Equals(Vec<Rule>),
    StrictEquals(Vec<Rule>),
    NotEquals(Vec<Rule>),
    StrictNotEquals(Vec<Rule>),
    GreaterThan(Vec<Rule>),
    LessThan(Vec<Rule>),
    GreaterThanEqual(Vec<Rule>),
    LessThanEqual(Vec<Rule>),
    
    // Logical operators
    And(Vec<Rule>),
    Or(Vec<Rule>),
    Not(Vec<Rule>),
    DoubleBang(Vec<Rule>),
    
    // Control operators
    If(Vec<Rule>),
    Ternary(Vec<Rule>),
    
    // Array operators
    Map(Vec<Rule>),
    Filter(Vec<Rule>),
    Reduce(Vec<Rule>),
    All(Vec<Rule>),
    None(Vec<Rule>),
    Some(Vec<Rule>),
    Merge(Vec<Rule>),
    
    // Missing operators
    Missing(Vec<Rule>),
    MissingSome(Vec<Rule>),
    
    // String operators
    In(Vec<Rule>),
    Cat(Vec<Rule>),
    Substr(Vec<Rule>),
    
    // Arithmetic operators
    Add(Vec<Rule>),
    Multiply(Vec<Rule>),
    Subtract(Vec<Rule>),
    Divide(Vec<Rule>),
    Modulo(Vec<Rule>),
    Max(Vec<Rule>),
    Min(Vec<Rule>),
    
    // Special operators
    Preserve(Vec<Rule>),
    
    // Literal value
    Value(Value),
}

impl Rule {
    pub fn from_value(value: &Value) -> Result<Self, Error> {
        match value {
            Value::Object(map) if map.len() == 1 => {
                let (op, args) = map.iter().next().unwrap();
                let args = match args {
                    Value::Array(arr) => arr.iter().map(Rule::from_value).collect::<Result<Vec<_>, _>>()?,
                    _ => vec![Rule::from_value(args)?],
                };
                
                match op.as_str() {
                    // Variable access
                    "var" => Ok(Rule::Var(args)),
                    
                    // Comparison operators
                    "==" => Ok(Rule::Equals(args)),
                    "===" => Ok(Rule::StrictEquals(args)),
                    "!=" => Ok(Rule::NotEquals(args)),
                    "!==" => Ok(Rule::StrictNotEquals(args)),
                    ">" => Ok(Rule::GreaterThan(args)),
                    "<" => Ok(Rule::LessThan(args)),
                    ">=" => Ok(Rule::GreaterThanEqual(args)),
                    "<=" => Ok(Rule::LessThanEqual(args)),
                    
                    // Logical operators
                    "and" => Ok(Rule::And(args)),
                    "or" => Ok(Rule::Or(args)),
                    "!" => Ok(Rule::Not(args)),
                    "!!" => Ok(Rule::DoubleBang(args)),
                    
                    // Control operators
                    "if" => Ok(Rule::If(args)),
                    "?:" => Ok(Rule::Ternary(args)),
                    
                    // Array operators
                    "map" => Ok(Rule::Map(args)),
                    "filter" => Ok(Rule::Filter(args)),
                    "reduce" => Ok(Rule::Reduce(args)),
                    "all" => Ok(Rule::All(args)),
                    "none" => Ok(Rule::None(args)),
                    "some" => Ok(Rule::Some(args)),
                    "merge" => Ok(Rule::Merge(args)),
                    
                    // Missing operators
                    "missing" => Ok(Rule::Missing(args)),
                    "missing_some" => Ok(Rule::MissingSome(args)),
                    
                    // String operators
                    "in" => Ok(Rule::In(args)),
                    "cat" => Ok(Rule::Cat(args)),
                    "substr" => Ok(Rule::Substr(args)),
                    
                    // Arithmetic operators
                    "+" => Ok(Rule::Add(args)),
                    "*" => Ok(Rule::Multiply(args)),
                    "-" => Ok(Rule::Subtract(args)),
                    "/" => Ok(Rule::Divide(args)),
                    "%" => Ok(Rule::Modulo(args)),
                    "max" => Ok(Rule::Max(args)),
                    "min" => Ok(Rule::Min(args)),
                    
                    // Special operators
                    "preserve" => Ok(Rule::Preserve(args)),
                    
                    _ => Err(Error::UnknownOperator(op.to_string())),
                }
            }
            _ => Ok(Rule::Value(value.clone())),
        }
    }

    pub fn apply(&self, data: &Value) -> Result<Value, Error> {
        match self {
            Rule::Value(value) => {
                if let Value::Array(arr) = value {
                    let mut result = Vec::with_capacity(arr.len());
                    for item in arr {
                        let item_rule = Rule::from_value(item)?;
                        result.push(item_rule.apply(data)?);
                    }
                    Ok(Value::Array(result))
                } else {
                    Ok(value.clone())
                }
            },
            _ => {
                let op = self.get_operator()?;
                let args = self.get_args();
                op.apply(args, data)
            }
        }
    }

    fn get_operator(&self) -> Result<Box<dyn Operator>, Error> {
        match self {
            // Variable access
            Rule::Var(_) => Ok(Box::new(VarOperator)),
            
            // Comparison operators
            Rule::Equals(_) => Ok(Box::new(EqualsOperator)),
            Rule::StrictEquals(_) => Ok(Box::new(StrictEqualsOperator)),
            Rule::NotEquals(_) => Ok(Box::new(NotEqualsOperator)),
            Rule::StrictNotEquals(_) => Ok(Box::new(StrictNotEqualsOperator)),
            Rule::GreaterThan(_) => Ok(Box::new(GreaterThanOperator)),
            Rule::LessThan(_) => Ok(Box::new(LessThanOperator)),
            Rule::GreaterThanEqual(_) => Ok(Box::new(GreaterThanEqualOperator)),
            Rule::LessThanEqual(_) => Ok(Box::new(LessThanEqualOperator)),
            
            // Logical operators
            Rule::And(_) => Ok(Box::new(AndOperator)),
            Rule::Or(_) => Ok(Box::new(OrOperator)),
            Rule::Not(_) => Ok(Box::new(NotOperator)),
            Rule::DoubleBang(_) => Ok(Box::new(DoubleBangOperator)),
            
            // Control operators
            Rule::If(_) => Ok(Box::new(IfOperator)),
            Rule::Ternary(_) => Ok(Box::new(TernaryOperator)),
            
            // Array operators
            Rule::Map(_) => Ok(Box::new(MapOperator)),
            Rule::Filter(_) => Ok(Box::new(FilterOperator)),
            Rule::Reduce(_) => Ok(Box::new(ReduceOperator)),
            Rule::All(_) => Ok(Box::new(AllOperator)),
            Rule::None(_) => Ok(Box::new(NoneOperator)),
            Rule::Some(_) => Ok(Box::new(SomeOperator)),
            Rule::Merge(_) => Ok(Box::new(MergeOperator)),
            
            // Missing operators
            Rule::Missing(_) => Ok(Box::new(MissingOperator)),
            Rule::MissingSome(_) => Ok(Box::new(MissingSomeOperator)),
            
            // String operators
            Rule::In(_) => Ok(Box::new(InOperator)),
            Rule::Cat(_) => Ok(Box::new(CatOperator)),
            Rule::Substr(_) => Ok(Box::new(SubstrOperator)),
            
            // Arithmetic operators
            Rule::Add(_) => Ok(Box::new(AddOperator)),
            Rule::Multiply(_) => Ok(Box::new(MultiplyOperator)),
            Rule::Subtract(_) => Ok(Box::new(SubtractOperator)),
            Rule::Divide(_) => Ok(Box::new(DivideOperator)),
            Rule::Modulo(_) => Ok(Box::new(ModuloOperator)),
            Rule::Max(_) => Ok(Box::new(MaxOperator)),
            Rule::Min(_) => Ok(Box::new(MinOperator)),
            
            // Special operators
            Rule::Preserve(_) => Ok(Box::new(PreserveOperator)),
            
            // Value is handled separately in apply_rule
            Rule::Value(_) => Err(Error::InvalidRule("Value does not have an operator".to_string())),
        }
    }

    fn get_args(&self) -> &[Rule] {
        match self {
            // Value (no args)
            Rule::Value(_) => &[],
            
            // Variable access
            Rule::Var(args) => args,
            
            // Comparison operators
            Rule::Equals(args) => args,
            Rule::StrictEquals(args) => args,
            Rule::NotEquals(args) => args,
            Rule::StrictNotEquals(args) => args,
            Rule::GreaterThan(args) => args,
            Rule::LessThan(args) => args,
            Rule::GreaterThanEqual(args) => args,
            Rule::LessThanEqual(args) => args,
            
            // Logical operators
            Rule::And(args) => args,
            Rule::Or(args) => args,
            Rule::Not(args) => args,
            Rule::DoubleBang(args) => args,
            
            // Control operators
            Rule::If(args) => args,
            Rule::Ternary(args) => args,
            
            // Array operators
            Rule::Map(args) => args,
            Rule::Filter(args) => args,
            Rule::Reduce(args) => args,
            Rule::All(args) => args,
            Rule::None(args) => args,
            Rule::Some(args) => args,
            Rule::Merge(args) => args,
            
            // Missing operators
            Rule::Missing(args) => args,
            Rule::MissingSome(args) => args,
            
            // String operators
            Rule::In(args) => args,
            Rule::Cat(args) => args,
            Rule::Substr(args) => args,
            
            // Arithmetic operators
            Rule::Add(args) => args,
            Rule::Multiply(args) => args,
            Rule::Subtract(args) => args,
            Rule::Divide(args) => args,
            Rule::Modulo(args) => args,
            Rule::Max(args) => args,
            Rule::Min(args) => args,
            
            // Special operators
            Rule::Preserve(args) => args,
        }
    }
}