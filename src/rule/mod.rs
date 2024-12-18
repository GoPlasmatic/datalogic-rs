mod operators;

use serde_json::Value;
use crate::Error;

pub use operators::*;

static VAR_OP: VarOperator = VarOperator;
static EQUALS_OP: EqualsOperator = EqualsOperator;
static STRICT_EQUALS_OP: StrictEqualsOperator = StrictEqualsOperator;
static NOT_EQUALS_OP: NotEqualsOperator = NotEqualsOperator;
static STRICT_NOT_EQUALS_OP: StrictNotEqualsOperator = StrictNotEqualsOperator;
static GREATER_THAN_OP: GreaterThanOperator = GreaterThanOperator;
static LESS_THAN_OP: LessThanOperator = LessThanOperator;
static GREATER_THAN_EQUAL_OP: GreaterThanEqualOperator = GreaterThanEqualOperator;
static LESS_THAN_EQUAL_OP: LessThanEqualOperator = LessThanEqualOperator;

static AND_OP: AndOperator = AndOperator;
static OR_OP: OrOperator = OrOperator;
static NOT_OP: NotOperator = NotOperator;
static DOUBLE_BANG_OP: DoubleBangOperator = DoubleBangOperator;

static IF_OP: IfOperator = IfOperator;
static TERNARY_OP: TernaryOperator = TernaryOperator;

static MAP_OP: MapOperator = MapOperator;
static FILTER_OP: FilterOperator = FilterOperator;
static REDUCE_OP: ReduceOperator = ReduceOperator;
static ALL_OP: AllOperator = AllOperator;
static NONE_OP: NoneOperator = NoneOperator;
static SOME_OP: SomeOperator = SomeOperator;
static MERGE_OP: MergeOperator = MergeOperator;

static MISSING_OP: MissingOperator = MissingOperator;
static MISSING_SOME_OP: MissingSomeOperator = MissingSomeOperator;

static IN_OP: InOperator = InOperator;
static CAT_OP: CatOperator = CatOperator;
static SUBSTR_OP: SubstrOperator = SubstrOperator;

static ADD_OP: AddOperator = AddOperator;
static MULTIPLY_OP: MultiplyOperator = MultiplyOperator;
static SUBTRACT_OP: SubtractOperator = SubtractOperator;
static DIVIDE_OP: DivideOperator = DivideOperator;
static MODULO_OP: ModuloOperator = ModuloOperator;
static MAX_OP: MaxOperator = MaxOperator;
static MIN_OP: MinOperator = MinOperator;

static PRESERVE_OP: PreserveOperator = PreserveOperator;


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

    Array(Vec<Rule>),
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
            },
            Value::Array(arr) => {
                Ok(Rule::Array(
                    arr.iter()
                        .map(Rule::from_value)
                        .collect::<Result<Vec<_>, _>>()?
                ))
            },
            _ => Ok(Rule::Value(value.clone())),
        }
    }

    pub fn apply(&self, data: &Value) -> Result<Value, Error> {
        match self {
            Rule::Value(value) => {
                Ok(value.clone())
            },
            Rule::Array(rules) => Ok(Value::Array(
                rules.iter()
                    .map(|rule| rule.apply(data))
                    .collect::<Result<Vec<_>, _>>()?
            )),
            Rule::Var(args) => VAR_OP.apply(args, data),

            Rule::Equals(args) => EQUALS_OP.apply(args, data),
            Rule::StrictEquals(args) => STRICT_EQUALS_OP.apply(args, data),
            Rule::NotEquals(args) => NOT_EQUALS_OP.apply(args, data),
            Rule::StrictNotEquals(args) => STRICT_NOT_EQUALS_OP.apply(args, data),

            Rule::GreaterThan(args) => GREATER_THAN_OP.apply(args, data),
            Rule::LessThan(args) => LESS_THAN_OP.apply(args, data),
            Rule::GreaterThanEqual(args) => GREATER_THAN_EQUAL_OP.apply(args, data),
            Rule::LessThanEqual(args) => LESS_THAN_EQUAL_OP.apply(args, data),

            Rule::And(args) => AND_OP.apply(args, data),
            Rule::Or(args) => OR_OP.apply(args, data),
            Rule::Not(args) => NOT_OP.apply(args, data),
            Rule::DoubleBang(args) => DOUBLE_BANG_OP.apply(args, data),

            Rule::If(args) => IF_OP.apply(args, data),
            Rule::Ternary(args) => TERNARY_OP.apply(args, data),

            Rule::Map(args) => MAP_OP.apply(args, data),
            Rule::Filter(args) => FILTER_OP.apply(args, data),
            Rule::Reduce(args) => REDUCE_OP.apply(args, data),
            Rule::All(args) => ALL_OP.apply(args, data),
            Rule::None(args) => NONE_OP.apply(args, data),
            Rule::Some(args) => SOME_OP.apply(args, data),
            Rule::Merge(args) => MERGE_OP.apply(args, data),

            Rule::Missing(args) => MISSING_OP.apply(args, data),
            Rule::MissingSome(args) => MISSING_SOME_OP.apply(args, data),
            
            Rule::In(args) => IN_OP.apply(args, data),
            Rule::Cat(args) => CAT_OP.apply(args, data),
            Rule::Substr(args) => SUBSTR_OP.apply(args, data),

            Rule::Add(args) => ADD_OP.apply(args, data),
            Rule::Multiply(args) => MULTIPLY_OP.apply(args, data),
            Rule::Subtract(args) => SUBTRACT_OP.apply(args, data),
            Rule::Divide(args) => DIVIDE_OP.apply(args, data),
            Rule::Modulo(args) => MODULO_OP.apply(args, data),
            Rule::Max(args) => MAX_OP.apply(args, data),
            Rule::Min(args) => MIN_OP.apply(args, data),

            Rule::Preserve(args) => PRESERVE_OP.apply(args, data),
        }
    }
}