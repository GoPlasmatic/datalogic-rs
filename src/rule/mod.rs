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
            _ => {
                let op = self.get_operator()?;
                let args = self.get_args();
                op.apply(args, data)
            }
        }
    }

    fn get_operator(&self) -> Result<&'static dyn Operator, Error> {
        match self {
            Rule::Var(_) => Ok(&VAR_OP),
            
            // Group comparison operators
            Rule::Equals(_) => Ok(&EQUALS_OP),
            Rule::StrictEquals(_) => Ok(&STRICT_EQUALS_OP),
            Rule::NotEquals(_) => Ok(&NOT_EQUALS_OP),
            Rule::StrictNotEquals(_) => Ok(&STRICT_NOT_EQUALS_OP),
            
            // Group arithmetic operators
            Rule::GreaterThan(_) => Ok(&GREATER_THAN_OP),
            Rule::LessThan(_) => Ok(&LESS_THAN_OP),
            Rule::GreaterThanEqual(_) => Ok(&GREATER_THAN_EQUAL_OP),
            Rule::LessThanEqual(_) => Ok(&LESS_THAN_EQUAL_OP),
            
            // Group logical operators
            Rule::And(_) => Ok(&AND_OP),
            Rule::Or(_) => Ok(&OR_OP),
            Rule::Not(_) => Ok(&NOT_OP),
            Rule::DoubleBang(_) => Ok(&DOUBLE_BANG_OP),
            
            // Group control operators
            Rule::If(_) => Ok(&IF_OP),
            Rule::Ternary(_) => Ok(&TERNARY_OP),
            
            // Group array operators
            Rule::Map(_) => Ok(&MAP_OP),
            Rule::Filter(_) => Ok(&FILTER_OP),
            Rule::Reduce(_) => Ok(&REDUCE_OP),
            Rule::All(_) => Ok(&ALL_OP),
            Rule::None(_) => Ok(&NONE_OP),
            Rule::Some(_) => Ok(&SOME_OP),
            Rule::Merge(_) => Ok(&MERGE_OP),
            
            // Group missing operators
            Rule::Missing(_) => Ok(&MISSING_OP),
            Rule::MissingSome(_) => Ok(&MISSING_SOME_OP),

            // Group string operators
            Rule::In(_) => Ok(&IN_OP),
            Rule::Cat(_) => Ok(&CAT_OP),
            Rule::Substr(_) => Ok(&SUBSTR_OP),

            // Group arithmetic operators
            Rule::Add(_) => Ok(&ADD_OP),
            Rule::Multiply(_) => Ok(&MULTIPLY_OP),
            Rule::Subtract(_) => Ok(&SUBTRACT_OP),
            Rule::Divide(_) => Ok(&DIVIDE_OP),
            Rule::Modulo(_) => Ok(&MODULO_OP),
            Rule::Max(_) => Ok(&MAX_OP),
            Rule::Min(_) => Ok(&MIN_OP),

            // Group special operators
            Rule::Preserve(_) => Ok(&PRESERVE_OP),
            
            Rule::Array(_) => Err(Error::InvalidRule("Array does not have an operator".to_string())),
            Rule::Value(_) => Err(Error::InvalidRule("Value does not have an operator".to_string())),
        }
    }

    fn get_args(&self) -> &[Rule] {
        match self {
            // Value (no args)
            Rule::Value(_) => &[],
            Rule::Array(args) => args,
            
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