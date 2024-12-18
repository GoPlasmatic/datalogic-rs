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


#[derive(Debug, Clone)]
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
    fn is_static(&self) -> bool {
        match self {
            Rule::Value(_) => true,
            Rule::Missing(_) | Rule::MissingSome(_) => false,

            Rule::Map(args) | 
            Rule::Filter(args) | 
            Rule::Reduce(args) |
            Rule::None(args) |
            Rule::Some(args) |
            Rule::All(args) => {
                if let [array, _] = args.as_slice() {
                    matches!(array, Rule::Value(Value::Array(_)))
                } else {
                    false
                }
            },
            Rule::Var(_) => false,
            // Rule::Var(args) |
            Rule::Array(args) |
            Rule::If(args) | 
            Rule::Equals(args) |
            Rule::StrictEquals(args) |
            Rule::NotEquals(args) |
            Rule::StrictNotEquals(args) |
            Rule::GreaterThan(args) |
            Rule::LessThan(args) |
            Rule::GreaterThanEqual(args) |
            Rule::LessThanEqual(args) |
            Rule::And(args) |
            Rule::Or(args) |
            Rule::Not(args) |
            Rule::DoubleBang(args) |
            Rule::Ternary(args) |
            Rule::Merge(args) |
            Rule::In(args) |
            Rule::Cat(args) |
            Rule::Substr(args) |
            Rule::Add(args) |
            Rule::Multiply(args) |
            Rule::Subtract(args) |
            Rule::Divide(args) |
            Rule::Modulo(args) |
            Rule::Max(args) |
            Rule::Min(args) |
            Rule::Preserve(args) => args.iter().all(|r| r.is_static()),
        }
    }

    fn optimize_args(args: &[Rule]) -> Result<Vec<Rule>, Error> {
        args.iter()
            .cloned()
            .map(Self::optimize_rule)
            .collect()
    }

    fn rebuild_with_args(rule: Rule, optimized: Vec<Rule>) -> Rule {
        match rule {
            Rule::Map(_) => Rule::Map(optimized),
            Rule::Filter(_) => Rule::Filter(optimized),
            Rule::Reduce(_) => Rule::Reduce(optimized),
            Rule::All(_) => Rule::All(optimized),
            Rule::None(_) => Rule::None(optimized),
            Rule::Some(_) => Rule::Some(optimized),
            Rule::Merge(_) => Rule::Merge(optimized),
            Rule::Missing(_) => Rule::Missing(optimized),
            Rule::MissingSome(_) => Rule::MissingSome(optimized),
            Rule::In(_) => Rule::In(optimized),
            Rule::Cat(_) => Rule::Cat(optimized),
            Rule::Substr(_) => Rule::Substr(optimized),
            Rule::Add(_) => Rule::Add(optimized),
            Rule::Multiply(_) => Rule::Multiply(optimized),
            Rule::Subtract(_) => Rule::Subtract(optimized),
            Rule::Divide(_) => Rule::Divide(optimized),
            Rule::Modulo(_) => Rule::Modulo(optimized),
            Rule::Max(_) => Rule::Max(optimized),
            Rule::Min(_) => Rule::Min(optimized),
            Rule::Equals(_) => Rule::Equals(optimized),
            Rule::StrictEquals(_) => Rule::StrictEquals(optimized),
            Rule::NotEquals(_) => Rule::NotEquals(optimized),
            Rule::StrictNotEquals(_) => Rule::StrictNotEquals(optimized),
            Rule::GreaterThan(_) => Rule::GreaterThan(optimized),
            Rule::LessThan(_) => Rule::LessThan(optimized),
            Rule::GreaterThanEqual(_) => Rule::GreaterThanEqual(optimized),
            Rule::LessThanEqual(_) => Rule::LessThanEqual(optimized),
            Rule::And(_) => Rule::And(optimized),
            Rule::Or(_) => Rule::Or(optimized),
            Rule::Not(_) => Rule::Not(optimized),
            Rule::DoubleBang(_) => Rule::DoubleBang(optimized),
            Rule::If(_) => Rule::If(optimized),
            Rule::Ternary(_) => Rule::Ternary(optimized),
            Rule::Preserve(_) => Rule::Preserve(optimized),
                        
            _ => rule
        }
    }

    fn optimize_rule(rule: Rule) -> Result<Rule, Error> {
        match rule {
            // Never optimize these
            Rule::Missing(_) | Rule::MissingSome(_) => Ok(rule),
            Rule::Value(_) => Ok(rule),

            // Handle static evaluation
            rule if rule.is_static() => {
                rule.apply(&Value::Null)
                    .map(Rule::Value)
                    .or(Ok(rule))
            },

            // Process arrays
            Rule::Array(args) => {
                let optimized = Self::optimize_args(&args)?;
                Ok(Rule::Array(optimized))
            },

            // Process operators
            Rule::Var(ref args) |
            Rule::Map(ref args) |
            Rule::Filter(ref args) |
            Rule::Reduce(ref args) |
            Rule::All(ref args) |
            Rule::None(ref args) |
            Rule::Some(ref args) |
            Rule::Merge(ref args) |
            Rule::In(ref args) |
            Rule::Cat(ref args) |
            Rule::Substr(ref args) |
            Rule::Add(ref args) |
            Rule::Multiply(ref args) |
            Rule::Subtract(ref args) |
            Rule::Divide(ref args) |
            Rule::Modulo(ref args) |
            Rule::Max(ref args) |
            Rule::Min(ref args) |
            Rule::Equals(ref args) |
            Rule::StrictEquals(ref args) |
            Rule::NotEquals(ref args) |
            Rule::StrictNotEquals(ref args) |
            Rule::GreaterThan(ref args) |
            Rule::LessThan(ref args) |
            Rule::GreaterThanEqual(ref args) |
            Rule::LessThanEqual(ref args) |
            Rule::And(ref args) |
            Rule::Or(ref args) |
            Rule::Not(ref args) |
            Rule::DoubleBang(ref args) |
            Rule::If(ref args) |
            Rule::Ternary(ref args) |
            Rule::Preserve(ref args) => {
                let optimized = Self::optimize_args(args)?;
                Ok(Self::rebuild_with_args(rule, optimized))
            },
        }
    }

    pub fn from_value(value: &Value) -> Result<Self, Error> {
        match value {
            Value::Object(map) if map.len() == 1 => {
                let (op, args) = map.iter().next().unwrap();
                let args = match args {
                    Value::Array(arr) => arr.iter()
                        .map(Rule::from_value)
                        .collect::<Result<Vec<_>, _>>()?,
                    _ => vec![Rule::from_value(args)?],
                };
                
                let rule = match op.as_str() {
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
                };

                // Optimize the rule before returning
                Self::optimize_rule(rule?)
            },
            Value::Array(arr) => {
                let rule = Rule::Array(
                    arr.iter()
                        .map(Rule::from_value)
                        .collect::<Result<Vec<_>, _>>()?
                );
                Self::optimize_rule(rule)
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