mod operators;

use serde_json::Value;
use crate::{Error, JsonLogicResult};

pub use operators::*;

static VAR_OP: VarOperator = VarOperator;
static LOGIC_OP: LogicOperator = LogicOperator;
static COMPARE_OP: CompareOperator = CompareOperator;
static ARITHMETIC_OP: ArithmeticOperator = ArithmeticOperator;

static IF_OP: IfOperator = IfOperator;
static TERNARY_OP: TernaryOperator = TernaryOperator;

static MAP_OP: MapOperator = MapOperator;
static FILTER_OP: FilterOperator = FilterOperator;
static REDUCE_OP: ReduceOperator = ReduceOperator;
static MERGE_OP: MergeOperator = MergeOperator;
static ARRAY_PREDICATE_OP: ArrayPredicateOperator = ArrayPredicateOperator;

static MISSING_OP: MissingOperator = MissingOperator;
static MISSING_SOME_OP: MissingSomeOperator = MissingSomeOperator;

static IN_OP: InOperator = InOperator;
static CAT_OP: CatOperator = CatOperator;
static SUBSTR_OP: SubstrOperator = SubstrOperator;

static PRESERVE_OP: PreserveOperator = PreserveOperator;

#[derive(Debug, Clone)]
pub enum ArgType {
    Array(Vec<Rule>),
    Single(Box<Rule>)
}

#[derive(Debug, Clone)]
pub enum Rule {
    Value(Value),
    Array(Vec<Rule>),

    Var(Box<Rule>, Option<Box<Rule>>),
    Compare(CompareType, Vec<Rule>),
    Arithmetic(ArithmeticType, ArgType),
    Logic(LogicType, Vec<Rule>),
    
    // Control operators
    If(Vec<Rule>),
    Ternary(Vec<Rule>),
    
    // String operators
    In(Box<Rule>, Box<Rule>),
    Cat(Vec<Rule>),
    Substr(Box<Rule>, Box<Rule>, Option<Box<Rule>>),
    
    // Array operators
    Map(Box<Rule>, Box<Rule>),
    Filter(Box<Rule>, Box<Rule>),
    Reduce(Box<Rule>, Box<Rule>, Box<Rule>),
    Merge(Vec<Rule>),
    ArrayPredicate(ArrayPredicateType, Box<Rule>, Box<Rule>),

    // Missing operators
    Missing(Vec<Rule>),
    MissingSome(Vec<Rule>),

    // Special operators
    Preserve(ArgType),
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
            Rule::Filter(array_rule, predicate) => {
                array_rule.is_static() && predicate.is_static()
            }
            Rule::ArrayPredicate(_, array_rule, predicate) => {
                array_rule.is_static() && predicate.is_static()
            }

            Rule::In(search, target) => {
                search.is_static() && target.is_static()
            }
            Rule::Substr(string, start, length) => {
                if length.is_none() {
                    string.is_static() && start.is_static()
                } else {
                    string.is_static() && start.is_static() && length.as_deref().unwrap().is_static()
                }
            }

            Rule::Compare(_, args) => {
                args.iter().all(|r| r.is_static())
            }
            Rule::Logic(_, args) => {
                args.iter().all(|r| r.is_static())
            }
            Rule::Arithmetic(_, args) => {
                match args {
                    ArgType::Array(arr) => arr.iter().all(|r| r.is_static()),
                    ArgType::Single(r) => r.is_static(),
                }
            }
            Rule::Preserve(args) => {
                match args {
                    ArgType::Array(arr) => arr.iter().all(|r| r.is_static()),
                    ArgType::Single(r) => r.is_static(),
                }
            }

            Rule::Array(args) |
            Rule::If(args) | 
            Rule::Ternary(args) |
            Rule::Merge(args) |
            Rule::Cat(args) => args.iter().all(|r| r.is_static()),
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
            Rule::Arithmetic(op, args) => {
                match args {
                    ArgType::Array(arr) => {
                        let optimized = Self::optimize_args(&arr)?;
                        Ok(Rule::Arithmetic(op, ArgType::Array(optimized)))
                    },
                    ArgType::Single(rule) => {
                        let optimized = Self::optimize_rule(*rule)?;
                        Ok(Rule::Arithmetic(op, ArgType::Single(Box::new(optimized))))
                    }
                }
            },
            Rule::Map(array_rule, predicate) => {
                let optimized_array_rule = Self::optimize_rule(*array_rule)?;
                let optimized_predicate = Self::optimize_rule(*predicate)?;
                Ok(Rule::Map(Box::new(optimized_array_rule), Box::new(optimized_predicate)))
            },
            Rule::ArrayPredicate(op, array_rule, predicate) => {
                let optimized_array_rule = Self::optimize_rule(*array_rule)?;
                let optimized_predicate = Self::optimize_rule(*predicate)?;
                Ok(Rule::ArrayPredicate(op, Box::new(optimized_array_rule), Box::new(optimized_predicate)))
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
            Rule::Preserve(args) => {
                match args {
                    ArgType::Array(arr) => {
                        let optimized = Self::optimize_args(&arr)?;
                        Ok(Rule::Preserve(ArgType::Array(optimized)))
                    },
                    ArgType::Single(rule) => {
                        let optimized = Self::optimize_rule(*rule)?;
                        Ok(Rule::Preserve(ArgType::Single(Box::new(optimized))))
                    }
                }
            }
            Rule::Compare(op, args) => {
                let optimized = Self::optimize_args(&args)?;
                Ok(Rule::Compare(op, optimized))
            },
            Rule::Logic(op, args) => {
                let optimized = Self::optimize_args(&args)?;
                Ok(Rule::Logic(op, optimized))
            },
            Rule::Merge(ref args) => {
                let optimized = Self::optimize_args(args)?;
                Ok(Rule::Merge(optimized))
            }
            Rule::Cat(ref args) => {
                let optimized = Self::optimize_args(args)?;
                Ok(Rule::Cat(optimized))
            }
            Rule::If(ref args) => {
                let optimized = Self::optimize_args(args)?;
                Ok(Rule::If(optimized))
            }
            Rule::Ternary(ref args) => {
                let optimized = Self::optimize_args(args)?;
                Ok(Rule::Ternary(optimized))
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
                let (op, args_raw) = map.iter().next().unwrap();
                let args = match args_raw {
                    Value::Array(arr) => arr.iter()
                        .map(Rule::from_value)
                        .collect::<Result<Vec<_>, _>>()?,
                    _ => vec![Rule::from_value(args_raw)?],
                };
                
                let rule = match op.as_str() {
                    // Variable access
                    "var" => Ok(Rule::Var(
                        Box::new(args.first().cloned().unwrap_or(Rule::Value(Value::Null))),
                        args.get(1).cloned().map(Box::new)
                    )),
                    
                    // Comparison operators
                    "==" => Ok(Rule::Compare(CompareType::Equals, args)),
                    "===" => Ok(Rule::Compare(CompareType::StrictEquals, args)),
                    "!=" => Ok(Rule::Compare(CompareType::NotEquals, args)),
                    "!==" => Ok(Rule::Compare(CompareType::StrictNotEquals, args)),
                    ">" => Ok(Rule::Compare(CompareType::GreaterThan, args)),
                    "<" => Ok(Rule::Compare(CompareType::LessThan, args)),
                    ">=" => Ok(Rule::Compare(CompareType::GreaterThanEqual, args)),
                    "<=" => Ok(Rule::Compare(CompareType::LessThanEqual, args)),
                    
                    // Logical operators
                    "and" => Ok(Rule::Logic(LogicType::And, args)),
                    "or" => Ok(Rule::Logic(LogicType::Or, args)),
                    "!" => Ok(Rule::Logic(LogicType::Not, args)),
                    "!!" => Ok(Rule::Logic(LogicType::DoubleBang, args)),
                    
                    // Control operators
                    "if" => Ok(Rule::If(args)),
                    "?:" => Ok(Rule::Ternary(args)),
                    
                    // Array operators
                    "map" => Ok(Rule::Map(
                        Box::new(args.first().cloned().unwrap_or(Rule::Value(Value::Null))),
                        Box::new(args.get(1).cloned().unwrap_or(Rule::Value(Value::Null)))
                    )),
                    "filter" => Ok(Rule::Filter(
                        Box::new(args.first().cloned().unwrap_or(Rule::Value(Value::Null))),
                        Box::new(args.get(1).cloned().unwrap_or(Rule::Value(Value::Null)))
                    )),
                    "reduce" => {
                        Ok(Rule::Reduce(
                            Box::new(args.first().cloned().unwrap_or(Rule::Value(Value::Null))),
                            Box::new(args.get(1).cloned().unwrap_or(Rule::Value(Value::Null))),
                            Box::new(args.get(2).cloned().unwrap_or(Rule::Value(Value::Null)))
                        ))
                    },
                    "all" => Ok(Rule::ArrayPredicate(
                        ArrayPredicateType::All,
                        Box::new(args.first().cloned().unwrap_or(Rule::Value(Value::Null))),
                        Box::new(args.get(1).cloned().unwrap_or(Rule::Value(Value::Null)))
                    )),
                    "none" => Ok(Rule::ArrayPredicate(
                        ArrayPredicateType::None,
                        Box::new(args.first().cloned().unwrap_or(Rule::Value(Value::Null))),
                        Box::new(args.get(1).cloned().unwrap_or(Rule::Value(Value::Null)))
                    )),
                    "some" => Ok(Rule::ArrayPredicate(
                        ArrayPredicateType::Some,
                        Box::new(args.first().cloned().unwrap_or(Rule::Value(Value::Null))),
                        Box::new(args.get(1).cloned().unwrap_or(Rule::Value(Value::Null)))
                    )),
                    "merge" => Ok(Rule::Merge(args)),
                    
                    // Missing operators
                    "missing" => Ok(Rule::Missing(args)),
                    "missing_some" => Ok(Rule::MissingSome(args)),
                    
                    // String operators
                    "in" => Ok(Rule::In(
                        Box::new(args.first().cloned().unwrap_or(Rule::Value(Value::Null))),
                        Box::new(args.get(1).cloned().unwrap_or(Rule::Value(Value::Null)))
                    )),
                    "cat" => Ok(Rule::Cat(args)),
                    "substr" => Ok(Rule::Substr(
                        Box::new(args.first().cloned().unwrap_or(Rule::Value(Value::Null))),
                        Box::new(args.get(1).cloned().unwrap_or(Rule::Value(Value::Null))),
                        args.get(2).cloned().map(Box::new)
                    )),
                    
                    // Arithmetic operators
                    "+" => {
                        let arg = match args_raw {
                            Value::Array(_) => ArgType::Array(args),
                            _ => ArgType::Single(Box::new(args[0].clone())),
                        };
                        Ok(Rule::Arithmetic(ArithmeticType::Add, arg))
                    },
                    "*" => {
                        let arg = match args_raw {
                            Value::Array(_) => ArgType::Array(args),
                            _ => ArgType::Single(Box::new(args[0].clone())),
                        };
                        Ok(Rule::Arithmetic(ArithmeticType::Multiply, arg))
                    },
                    "-" => {
                        let arg = match args_raw {
                            Value::Array(_) => ArgType::Array(args),
                            _ => ArgType::Single(Box::new(args[0].clone())),
                        };
                        Ok(Rule::Arithmetic(ArithmeticType::Subtract, arg))
                    },
                    "/" => {
                        let arg = match args_raw {
                            Value::Array(_) => ArgType::Array(args),
                            _ => ArgType::Single(Box::new(args[0].clone())),
                        };
                        Ok(Rule::Arithmetic(ArithmeticType::Divide, arg))
                    },
                    "%" => {
                        let arg = match args_raw {
                            Value::Array(_) => ArgType::Array(args),
                            _ => ArgType::Single(Box::new(args[0].clone())),
                        };
                        Ok(Rule::Arithmetic(ArithmeticType::Modulo, arg))
                    },
                    "max" => {
                        let arg = match args_raw {
                            Value::Array(_) => ArgType::Array(args),
                            _ => ArgType::Single(Box::new(args[0].clone())),
                        };
                        Ok(Rule::Arithmetic(ArithmeticType::Max, arg))
                    },
                    "min" => {
                        let arg = match args_raw {
                            Value::Array(_) => ArgType::Array(args),
                            _ => ArgType::Single(Box::new(args[0].clone())),
                        };
                        Ok(Rule::Arithmetic(ArithmeticType::Min, arg))
                    },
                    "preserve" => {
                        let arg = match args_raw {
                            Value::Array(_) => ArgType::Array(args),
                            _ => ArgType::Single(Box::new(args[0].clone())),
                        };
                        Ok(Rule::Preserve(arg))
                    },
                    
                    _ => Err(Error::UnknownOperator(op.to_string())),
                };
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

            Rule::Compare(op, args) => COMPARE_OP.apply(args, data, op),
            Rule::Logic(op, args) => LOGIC_OP.apply(args, data, op),
            Rule::Arithmetic(op, args) => ARITHMETIC_OP.apply(args, data, op),

            Rule::If(args) => IF_OP.apply(args, data),
            Rule::Ternary(args) => TERNARY_OP.apply(args, data),

            Rule::Map(array_rule, predicate) => MAP_OP.apply(array_rule, predicate, data),
            Rule::Filter(array_rule, predicate) => FILTER_OP.apply(array_rule, predicate, data),
            Rule::Reduce(array_rule, reducer_rule, initial_rule) => REDUCE_OP.apply(array_rule, reducer_rule, initial_rule, data),
            Rule::Merge(args) => MERGE_OP.apply(args, data),
            Rule::ArrayPredicate(op, array_rule, predicate) => ARRAY_PREDICATE_OP.apply(array_rule, predicate, data, op),

            Rule::Missing(args) => MISSING_OP.apply(args, data),
            Rule::MissingSome(args) => MISSING_SOME_OP.apply(args, data),
            
            Rule::In(search, target) => IN_OP.apply(search, target, data),
            Rule::Cat(args) => CAT_OP.apply(args, data),
            Rule::Substr(string, start, length) => SUBSTR_OP.apply(string, start, length.as_deref(), data),

            Rule::Preserve(args) => PRESERVE_OP.apply(args, data),
        }
    }
}