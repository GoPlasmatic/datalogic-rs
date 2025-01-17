mod operators;

use serde_json::Value;
use crate::{Error, JsonLogicResult};

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
    Var(Box<Rule>, Option<Box<Rule>>),
    
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
    Map(Box<Rule>, Box<Rule>),
    Filter(Box<Rule>, Box<Rule>),
    Reduce(Box<Rule>, Box<Rule>, Box<Rule>),
    All(Box<Rule>, Box<Rule>),
    None(Box<Rule>, Box<Rule>),
    Some(Box<Rule>, Box<Rule>),
    Merge(Vec<Rule>),
    
    // Missing operators
    Missing(Vec<Rule>),
    MissingSome(Vec<Rule>),
    
    // String operators
    In(Box<Rule>, Box<Rule>),
    Cat(Vec<Rule>),
    Substr(Box<Rule>, Box<Rule>, Option<Box<Rule>>),
    
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
    #[inline(always)]
    fn is_static(&self) -> bool {
        match self {
            Rule::Value(_) => true,
            Rule::Missing(_) | Rule::MissingSome(_) => false,
            Rule::Var(_, _) => false,

            Rule::Map(array_rule, mapper) => {
                array_rule.is_static() && mapper.is_static()
            }
            Rule::Reduce(array_rule, reducer, initial) => {
                array_rule.is_static() && reducer.is_static() && initial.is_static()
            }
            Rule::Filter(array_rule, predicate) |
            Rule::All(array_rule, predicate) |
            Rule::None(array_rule, predicate) |
            Rule::Some(array_rule, predicate) => {
                array_rule.is_static() && predicate.is_static()
            }

            Rule::In(search, target) => {
                search.is_static() && target.is_static()
            }
            Rule::Substr(string, start, length) => {
                if length.is_none() {
                    return string.is_static() && start.is_static();
                } else {
                    return string.is_static() && start.is_static() && length.as_deref().unwrap().is_static();
                }
            }

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
            Rule::Cat(args) |
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

    #[inline]
    fn optimize_args(args: &[Rule]) -> Result<Vec<Rule>, Error> {
        args.iter()
            .cloned()
            .map(Self::optimize_rule)
            .collect()
    }

    #[inline]
    fn rebuild_with_args(rule: Rule, optimized: Vec<Rule>) -> Rule {
        match rule {
            Rule::Merge(_) => Rule::Merge(optimized),
            Rule::Missing(_) => Rule::Missing(optimized),
            Rule::MissingSome(_) => Rule::MissingSome(optimized),
            Rule::Cat(_) => Rule::Cat(optimized),
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
            Rule::Ternary(_) => Rule::Ternary(optimized),
            Rule::Preserve(_) => Rule::Preserve(optimized),
                        
            _ => rule
        }
    }

    #[inline]
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

            Rule::Var(path, default) => {
                let optimized_path = Self::optimize_rule(*path)?;
                let optimized_default = default.map(|d| Self::optimize_rule(*d)).transpose()?;
                Ok(Rule::Var(Box::new(optimized_path), optimized_default.map(Box::new)))
            },

            Rule::Map(array_rule, predicate) => {
                let optimized_array_rule = Self::optimize_rule(*array_rule)?;
                let optimized_predicate = Self::optimize_rule(*predicate)?;
                Ok(Rule::Map(Box::new(optimized_array_rule), Box::new(optimized_predicate)))
            },
            Rule::All(array_rule, predicate ) => {
                let optimized_array_rule = Self::optimize_rule(*array_rule)?;
                let optimized_predicate = Self::optimize_rule(*predicate)?;
                Ok(Rule::All(Box::new(optimized_array_rule), Box::new(optimized_predicate)))
            },
            Rule::None(array_rule, predicate) => {
                let optimized_array_rule = Self::optimize_rule(*array_rule)?;
                let optimized_predicate = Self::optimize_rule(*predicate)?;
                Ok(Rule::None(Box::new(optimized_array_rule), Box::new(optimized_predicate)))
            },
            Rule::Some(array_rule, predicate) => {
                let optimized_array_rule = Self::optimize_rule(*array_rule)?;
                let optimized_predicate = Self::optimize_rule(*predicate)?;
                Ok(Rule::Some(Box::new(optimized_array_rule), Box::new(optimized_predicate)))
            },
            Rule::Filter(array_rule, predicate) => {
                let optimized_array_rule = Self::optimize_rule(*array_rule)?;
                let optimized_predicate = Self::optimize_rule(*predicate)?;
                Ok(Rule::Filter(Box::new(optimized_array_rule), Box::new(optimized_predicate)))
            },
            Rule::Reduce(array_rule, predicate, initial) => {
                let optimized_array_rule = Self::optimize_rule(*array_rule)?;
                let optimized_initial = Self::optimize_rule(*initial)?;
                let optimized_predicate = Self::optimize_rule(*predicate)?;
                Ok(Rule::Reduce(Box::new(optimized_array_rule), Box::new(optimized_predicate), Box::new(optimized_initial)))
            },

            Rule::In(search, target) => {
                let optimized_search = Self::optimize_rule(*search)?;
                let optimized_target = Self::optimize_rule(*target)?;
                Ok(Rule::In(Box::new(optimized_search), Box::new(optimized_target)))
            },
            Rule::Substr(string, start, length) => {
                let optimized_string = Self::optimize_rule(*string)?;
                let optimized_start = Self::optimize_rule(*start)?;
                let optimized_length = length.map(|l| Self::optimize_rule(*l)).transpose()?;
                Ok(Rule::Substr(Box::new(optimized_string), Box::new(optimized_start), optimized_length.map(Box::new)))
            },
            // Process operators
            Rule::Merge(ref args) |
            Rule::Cat(ref args) |
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

    /// Creates a new `Rule` from a JSON Value
    ///
    /// Parses a serde_json::Value into a Rule that can be evaluated. The value must follow
    /// the JSONLogic specification format.
    ///
    /// ## Arguments
    /// * `value` - A JSON value representing the rule. Must be a valid JSONLogic expression.
    ///
    /// ## Returns
    /// * `Result<Rule, Error>` - A Result containing either the parsed Rule or an error
    ///
    /// ## Examples
    ///
    /// Basic usage:
    /// ```rust
    /// use datalogic_rs::Rule;
    /// use serde_json::json;
    ///
    /// let rule = Rule::from_value(&json!({"==": [1, 1]})).unwrap();
    /// assert!(rule.apply(&json!(null)).unwrap().as_bool().unwrap());
    /// ```
    ///
    /// Complex nested rules:
    /// ```rust
    /// use datalogic_rs::Rule;
    /// use serde_json::json;
    ///
    /// let rule = Rule::from_value(&json!({
    ///     "and": [
    ///         {">": [{"var": "age"}, 18]},
    ///         {"<": [{"var": "age"}, 65]}
    ///     ]
    /// })).unwrap();
    /// ```
    ///
    /// Error handling:
    /// ```rust
    /// use datalogic_rs::Rule;
    /// use serde_json::json;
    ///
    /// let result = Rule::from_value(&json!({"invalid_op": []}));
    /// assert!(result.is_err());
    /// ```
    ///
    /// See also: [`from_str`](Rule::from_str), [`apply`](Rule::apply)
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
                    "var" => Ok(Rule::Var(
                        Box::new(args.get(0).cloned().unwrap_or(Rule::Value(Value::Null))),
                        args.get(1).cloned().map(Box::new)
                    )),
                    
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
                    "map" => Ok(Rule::Map(
                        Box::new(args.get(0).cloned().unwrap_or(Rule::Value(Value::Null))),
                        Box::new(args.get(1).cloned().unwrap_or(Rule::Value(Value::Null)))
                    )),
                    "filter" => Ok(Rule::Filter(
                        Box::new(args.get(0).cloned().unwrap_or(Rule::Value(Value::Null))),
                        Box::new(args.get(1).cloned().unwrap_or(Rule::Value(Value::Null)))
                    )),
                    "reduce" => Ok(Rule::Reduce(
                        Box::new(args.get(0).cloned().unwrap_or(Rule::Value(Value::Null))),
                        Box::new(args.get(1).cloned().unwrap_or(Rule::Value(Value::Null))),
                        Box::new(args.get(2).cloned().unwrap_or(Rule::Value(Value::Null)))
                    )),
                    "all" => Ok(Rule::All(
                        Box::new(args.get(0).cloned().unwrap_or(Rule::Value(Value::Null))),
                        Box::new(args.get(1).cloned().unwrap_or(Rule::Value(Value::Null)))
                    )),
                    "none" => Ok(Rule::None(
                        Box::new(args.get(0).cloned().unwrap_or(Rule::Value(Value::Null))),
                        Box::new(args.get(1).cloned().unwrap_or(Rule::Value(Value::Null)))
                    )),
                    "some" => Ok(Rule::Some(
                        Box::new(args.get(0).cloned().unwrap_or(Rule::Value(Value::Null))),
                        Box::new(args.get(1).cloned().unwrap_or(Rule::Value(Value::Null)))
                    )),
                    "merge" => Ok(Rule::Merge(args)),
                    
                    // Missing operators
                    "missing" => Ok(Rule::Missing(args)),
                    "missing_some" => Ok(Rule::MissingSome(args)),
                    
                    // String operators
                    "in" => Ok(Rule::In(
                        Box::new(args.get(0).cloned().unwrap_or(Rule::Value(Value::Null))),
                        Box::new(args.get(1).cloned().unwrap_or(Rule::Value(Value::Null)))
                    )),
                    "cat" => Ok(Rule::Cat(args)),
                    "substr" => Ok(Rule::Substr(
                        Box::new(args.get(0).cloned().unwrap_or(Rule::Value(Value::Null))),
                        Box::new(args.get(1).cloned().unwrap_or(Rule::Value(Value::Null))),
                        args.get(2).cloned().map(Box::new)
                    )),
                    
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

    pub fn apply(&self, data: &Value) -> JsonLogicResult {
        match self {
            Rule::Value(value) => {
                Ok(value.clone())
            },
            Rule::Array(rules) => Ok(Value::Array(
                rules.iter()
                    .map(|rule| rule.apply(data))
                    .collect::<Result<Vec<_>, _>>()?
            )),
            Rule::Var(path, default) => VAR_OP.apply(path, default.as_deref(), data),

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

            Rule::Map(array_rule, predicate) => MAP_OP.apply(array_rule, predicate, data),
            Rule::Filter(array_rule, predicate) => FILTER_OP.apply(array_rule, predicate, data),
            Rule::Reduce(array_rule, reducer_rule, initial_rule) => REDUCE_OP.apply(array_rule, reducer_rule, initial_rule, data),
            Rule::All(array_rule, predicate) => ALL_OP.apply(array_rule, predicate, data),
            Rule::None(array_rule, predicate) => NONE_OP.apply(array_rule, predicate, data),
            Rule::Some(array_rule, predicate) => SOME_OP.apply(array_rule, predicate, data),
            Rule::Merge(args) => MERGE_OP.apply(args, data),

            Rule::Missing(args) => MISSING_OP.apply(args, data),
            Rule::MissingSome(args) => MISSING_SOME_OP.apply(args, data),
            
            Rule::In(search, target) => IN_OP.apply(search, target, data),
            Rule::Cat(args) => CAT_OP.apply(args, data),
            Rule::Substr(string, start, length) => SUBSTR_OP.apply(string, start, length.as_deref(), data),

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