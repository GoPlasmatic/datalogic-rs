mod operators;

use serde_json::Value;
use crate::Error;
use crate::JsonLogic;
use std::borrow::Cow;

pub use operators::*;

// Value access operators
static VAL_OP: ValOperator = ValOperator;
static VAR_OP: VarOperator = VarOperator;
static EXISTS_OP: ExistsOperator = ExistsOperator;

// Logic and comparison operators
static LOGIC_OP: LogicOperator = LogicOperator;
static COMPARE_OP: CompareOperator = CompareOperator;
static ARITHMETIC_OP: ArithmeticOperator = ArithmeticOperator;

// Control flow operators
static IF_OP: IfOperator = IfOperator;
static COALESCE_OP: CoalesceOperator = CoalesceOperator;
static TRY_OP: TryOperator = TryOperator;

// Array operators
static MAP_OP: MapOperator = MapOperator;
static FILTER_OP: FilterOperator = FilterOperator;
static REDUCE_OP: ReduceOperator = ReduceOperator;
static MERGE_OP: MergeOperator = MergeOperator;
static ARRAY_PREDICATE_OP: ArrayPredicateOperator = ArrayPredicateOperator;

// Missing operators
static MISSING_OP: MissingOperator = MissingOperator;
static MISSING_SOME_OP: MissingSomeOperator = MissingSomeOperator;

// String operators
static IN_OP: InOperator = InOperator;
static CAT_OP: CatOperator = CatOperator;
static SUBSTR_OP: SubstrOperator = SubstrOperator;

// Custom operators
static CUSTOM_OP_WRAPPER: CustomOperatorWrapper = CustomOperatorWrapper;

#[derive(Debug, Clone)]
pub enum ArgType {
    Unary(Box<Rule>),
    Multiple(Vec<Rule>),
}

/// # Rule Construction
/// 
/// The `Rule` struct represents a parsed JSON Logic expression.
/// This struct encapsulates logic for evaluating expressions dynamically.
/// 
/// ## Example
/// ```rust
/// use datalogic_rs::Rule;
/// use serde_json::json;
/// 
/// let rule = Rule::from_value(&json!({"==": [1, 1]})).unwrap();
/// 
/// ```
#[derive(Debug, Clone)]
pub enum Rule {
    Value(Value),
    Array(Vec<Rule>),

    Val(ArgType),
    Var(Box<Rule>, Option<Box<Rule>>),
    Compare(CompareType, Vec<Rule>),
    Arithmetic(ArithmeticType, ArgType),
    Logic(LogicType, ArgType),
    Exists(ArgType),
    
    // Control operators
    If(ArgType),
    Coalesce(Vec<Rule>),
    
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
    Throw(Box<Rule>),
    Try(ArgType),

    Custom(String, Vec<Rule>),
}

/// Trait for optimizing rules
pub trait Optimizable {
    /// Optimize a rule by evaluating static expressions and simplifying where possible
    fn optimize(self) -> Result<Self, Error> where Self: Sized;
}

impl Optimizable for Rule {
    fn optimize(self) -> Result<Self, Error> {
        Self::optimize_rule(self)
    }
}

impl Rule {
    // Helper function to create ArgType from Value and args
    fn create_arg_type(args_raw: &Value, args: Vec<Rule>) -> ArgType {
        match args_raw {
            Value::Array(_) => ArgType::Multiple(args),
            _ => ArgType::Unary(Box::new(args[0].clone())),
        }
    }

    // Helper function to get an argument with a default value
    fn get_arg(args: &[Rule], index: usize) -> Rule {
        args.get(index).cloned().unwrap_or(Rule::Value(Value::Null))
    }

    // Helper function to get a boxed argument with a default value
    fn get_boxed_arg(args: &[Rule], index: usize) -> Box<Rule> {
        Box::new(Self::get_arg(args, index))
    }

    #[inline]
    fn is_static(&self) -> bool {
        match self {
            Rule::Value(_) => true,
            Rule::Array(args) => args.iter().all(|r| r.is_static()),
            Rule::Throw(rule) => rule.is_static(),
            
            // Delegate to operator-specific implementations
            Rule::Val(_) => VAL_OP.is_static(self),
            Rule::Var(_, _) => VAR_OP.is_static(self),
            Rule::Exists(_) => EXISTS_OP.is_static(self),
            Rule::Compare(_, _) => COMPARE_OP.is_static(self),
            Rule::Logic(_, _) => LOGIC_OP.is_static(self),
            Rule::Arithmetic(_, _) => ARITHMETIC_OP.is_static(self),
            Rule::If(_) => IF_OP.is_static(self),
            Rule::Coalesce(_) => COALESCE_OP.is_static(self),
            Rule::Map(_, _) => MAP_OP.is_static(self),
            Rule::Filter(_, _) => FILTER_OP.is_static(self),
            Rule::Reduce(_, _, _) => REDUCE_OP.is_static(self),
            Rule::Merge(_) => MERGE_OP.is_static(self),
            Rule::ArrayPredicate(_, _, _) => ARRAY_PREDICATE_OP.is_static(self),
            Rule::Missing(_) => MISSING_OP.is_static(self),
            Rule::MissingSome(_) => MISSING_SOME_OP.is_static(self),
            Rule::In(_, _) => IN_OP.is_static(self),
            Rule::Cat(_) => CAT_OP.is_static(self),
            Rule::Substr(_, _, _) => SUBSTR_OP.is_static(self),
            Rule::Try(_) => TRY_OP.is_static(self),
            Rule::Custom(_, _) => CUSTOM_OP_WRAPPER.is_static(self),
        }
    }

    fn optimize_args(args: &[Rule]) -> Result<Vec<Rule>, Error> {
        args.iter()
            .cloned()
            .map(Self::optimize_rule)
            .collect()
    }

    fn optimize_arg_type(arg_type: ArgType) -> Result<ArgType, Error> {
        match arg_type {
            ArgType::Multiple(arr) => {
                let optimized = Self::optimize_args(&arr)?;
                Ok(ArgType::Multiple(optimized))
            },
            ArgType::Unary(rule) => {
                let optimized = Self::optimize_rule(*rule)?;
                Ok(ArgType::Unary(Box::new(optimized)))
            }
        }
    }

    fn optimize_rule(rule: Rule) -> Result<Rule, Error> {
        match rule {
            // Never optimize these
            Rule::Missing(_) | Rule::MissingSome(_) | Rule::Value(_) => Ok(rule),

            // Handle static evaluation
            rule if rule.is_static() => {
                rule.apply(&Value::Null, &Value::Null, "")
                    .map(|v| Rule::Value(v.into_owned()))
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
            Rule::Val(arg) => {
                let optimized = Self::optimize_arg_type(arg)?;
                Ok(Rule::Val(optimized))
            },
            Rule::Exists(arg) => {
                let optimized = Self::optimize_arg_type(arg)?;
                Ok(Rule::Exists(optimized))
            },
            Rule::Arithmetic(op, arg) => {
                let optimized = Self::optimize_arg_type(arg)?;
                Ok(Rule::Arithmetic(op, optimized))
            },
            Rule::Logic(op, arg) => {
                let optimized = Self::optimize_arg_type(arg)?;
                Ok(Rule::Logic(op, optimized))
            },
            Rule::If(arg) => {
                let optimized = Self::optimize_arg_type(arg)?;
                Ok(Rule::If(optimized))
            },
            Rule::Try(arg) => {
                let optimized = Self::optimize_arg_type(arg)?;
                Ok(Rule::Try(optimized))
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
            Rule::Compare(op, args) => {
                let optimized = Self::optimize_args(&args)?;
                Ok(Rule::Compare(op, optimized))
            },
            Rule::Merge(args) => {
                let optimized = Self::optimize_args(&args)?;
                Ok(Rule::Merge(optimized))
            },
            Rule::Cat(args) => {
                let optimized = Self::optimize_args(&args)?;
                Ok(Rule::Cat(optimized))
            },
            Rule::Coalesce(args) => {
                let optimized = Self::optimize_args(&args)?;
                Ok(Rule::Coalesce(optimized))
            },
            Rule::Throw(rule) => {
                let optimized = Self::optimize_rule(*rule)?;
                Ok(Rule::Throw(Box::new(optimized)))
            },
            Rule::Custom(name, args) => {
                let optimized = Self::optimize_args(&args)?;
                Ok(Rule::Custom(name, optimized))
            },
        }
    }

    /// Parses a JSON Value into a `Rule` that can be evaluated.
    /// 
    /// This function accepts a JSONLogic-compliant expression and converts it
    /// into an internal `Rule` representation.
    /// 
    /// ## Arguments
    /// - `value`: A JSON value representing the rule.
    /// 
    /// ## Returns
    /// - `Result<Rule, Error>`: A valid rule or an error if parsing fails.
    /// 
    /// ## Example
    /// ```rust
    /// use datalogic_rs::Rule;
    /// use serde_json::json;
    /// 
    /// let rule = Rule::from_value(&json!({">": [{"var": "salary"}, 50000]})).unwrap();
    /// ```
    pub fn from_value(value: &Value) -> Result<Self, Error> {
        match value {
            Value::Object(map) if map.len() == 1 => {
                let (op, args_raw) = map.iter().next().unwrap();
                let args = if op != "preserve" {
                    match args_raw {
                        Value::Array(arr) => arr.iter()
                            .map(Rule::from_value)
                            .collect::<Result<Vec<_>, _>>()?,
                        _ => vec![Rule::from_value(args_raw)?],
                    }
                } else {
                    vec![]
                };
                
                let rule = match op.as_str() {
                    // Variable access
                    "var" => Ok(Rule::Var(
                        Self::get_boxed_arg(&args, 0),
                        args.get(1).cloned().map(Box::new)
                    )),
                    "val" => Ok(Rule::Val(Self::create_arg_type(args_raw, args))),
                    "exists" => Ok(Rule::Exists(Self::create_arg_type(args_raw, args))),
                    
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
                    "and" => Ok(Rule::Logic(LogicType::And, Self::create_arg_type(args_raw, args))),
                    "or" => Ok(Rule::Logic(LogicType::Or, Self::create_arg_type(args_raw, args))),
                    "!" => Ok(Rule::Logic(LogicType::Not, Self::create_arg_type(args_raw, args))),
                    "!!" => Ok(Rule::Logic(LogicType::DoubleBang, Self::create_arg_type(args_raw, args))),
                    
                    // Control operators
                    "if" | "?:" => Ok(Rule::If(Self::create_arg_type(args_raw, args))),
                    "??" => Ok(Rule::Coalesce(args)),
                    
                    // Array operators
                    "map" => {
                        if let Value::Array(_) = args_raw {
                            Ok(Rule::Map(
                                Self::get_boxed_arg(&args, 0),
                                Self::get_boxed_arg(&args, 1)
                            ))
                        } else {
                            Ok(Rule::Map(Box::new(Rule::Value(Value::Null)), Box::new(Rule::Value(Value::Null))))
                        }
                    },
                    "filter" => {
                        if let Value::Array(_) = args_raw {
                            Ok(Rule::Filter(
                                Self::get_boxed_arg(&args, 0),
                                Self::get_boxed_arg(&args, 1)
                            ))
                        } else {
                            Ok(Rule::Filter(Box::new(Rule::Value(Value::Null)), Box::new(Rule::Value(Value::Null))))
                        }
                    },
                    "reduce" => {
                        let array = Self::get_arg(&args, 0);
                        let predicate = Self::get_arg(&args, 1);
                        let initial = Self::get_arg(&args, 2);

                        // Try to desugar if predicate is a simple arithmetic operation
                        if let Rule::Arithmetic(op, ArgType::Multiple(args)) = &predicate {
                            if args.len() == 2 && is_flat_arithmetic_predicate(&predicate) {
                                let merged = Rule::Merge(vec![initial, array]);

                                // Convert to direct arithmetic operation
                                return Ok(Rule::Arithmetic(
                                    *op,
                                    ArgType::Unary(Box::new(merged))
                                ));
                            }
                        }

                        // Fall back to regular reduce if desugaring not possible
                        Ok(Rule::Reduce(
                            Box::new(array),
                            Box::new(predicate),
                            Box::new(initial)
                        ))
                    },
                    "all" if args_raw.is_array() => Ok(Rule::ArrayPredicate(
                        ArrayPredicateType::All,
                        Self::get_boxed_arg(&args, 0),
                        Self::get_boxed_arg(&args, 1)
                    )),
                    "none" if args_raw.is_array() => Ok(Rule::ArrayPredicate(
                        ArrayPredicateType::None,
                        Self::get_boxed_arg(&args, 0),
                        Self::get_boxed_arg(&args, 1)
                    )),
                    "some" if args_raw.is_array() => Ok(Rule::ArrayPredicate(
                        ArrayPredicateType::Some,
                        Self::get_boxed_arg(&args, 0),
                        Self::get_boxed_arg(&args, 1)
                    )),
                    "all" | "none" | "some" => Ok(Rule::ArrayPredicate(
                        ArrayPredicateType::Invalid,
                        Box::new(Rule::Value(Value::Null)),
                        Box::new(Rule::Value(Value::Null))
                    )),
                    "merge" => Ok(Rule::Merge(args)),
                    
                    // Missing operators
                    "missing" => Ok(Rule::Missing(args)),
                    "missing_some" => Ok(Rule::MissingSome(args)),
                    
                    // String operators
                    "in" => Ok(Rule::In(
                        Self::get_boxed_arg(&args, 0),
                        Self::get_boxed_arg(&args, 1)
                    )),
                    "cat" => Ok(Rule::Cat(args)),
                    "substr" => Ok(Rule::Substr(
                        Self::get_boxed_arg(&args, 0),
                        Self::get_boxed_arg(&args, 1),
                        args.get(2).cloned().map(Box::new)
                    )),
                    
                    // Arithmetic operators
                    "+" => Ok(Rule::Arithmetic(ArithmeticType::Add, Self::create_arg_type(args_raw, args))),
                    "*" => Ok(Rule::Arithmetic(ArithmeticType::Multiply, Self::create_arg_type(args_raw, args))),
                    "-" => Ok(Rule::Arithmetic(ArithmeticType::Subtract, Self::create_arg_type(args_raw, args))),
                    "/" => Ok(Rule::Arithmetic(ArithmeticType::Divide, Self::create_arg_type(args_raw, args))),
                    "%" => Ok(Rule::Arithmetic(ArithmeticType::Modulo, Self::create_arg_type(args_raw, args))),
                    "max" => Ok(Rule::Arithmetic(ArithmeticType::Max, Self::create_arg_type(args_raw, args))),
                    "min" => Ok(Rule::Arithmetic(ArithmeticType::Min, Self::create_arg_type(args_raw, args))),
                    "preserve" => {
                        let arg = Rule::Value(args_raw.clone());
                        Ok(arg)
                    },
                    "throw" => Ok(Rule::Throw(Box::new(args[0].clone()))),
                    "try" => Ok(Rule::Try(Self::create_arg_type(args_raw, args))),
                    _ => {
                        let json_logic = JsonLogic::global();
                        if json_logic.get_operator(op).is_some() {
                            Ok(Rule::Custom(op.to_string(), args))
                        } else {
                            Err(Error::InvalidArguments(op.to_string()))
                        }
                    },
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

    /// Create a rule from a JSON string
    pub fn from_json(json: &str) -> Result<Self, Error> {
        let value: Value = match serde_json::from_str(json) {
            Ok(v) => v,
            Err(e) => return Err(Error::InvalidExpression(e.to_string())),
        };
        let rule = Self::from_value(&value)?;
        rule.optimize()
    }

    /// Create an optimized rule from a JSON Value
    pub fn from_value_optimized(value: &Value) -> Result<Self, Error> {
        let rule = Self::from_value(value)?;
        rule.optimize()
    }

    #[inline]
    pub fn apply<'a>(&'a self, context: &'a Value, root: &'a Value, rpath: &str) -> Result<Cow<'a, Value>, Error> {
        match self {
            Rule::Value(value) => Ok(Cow::Borrowed(value)),
            Rule::Array(rules) => {
                let mut results = Vec::with_capacity(rules.len());
                for rule in rules {
                    results.push(rule.apply(context, root, rpath)?.into_owned());
                }
                Ok(Cow::Owned(Value::Array(results)))
            }
            Rule::Var(path, default) => VAR_OP.apply(path, default.as_deref(), context, root, rpath),
            Rule::Val(path) => VAL_OP.apply(path, context, root, rpath),
            Rule::Exists(path) => EXISTS_OP.apply(path, context, root, rpath),

            Rule::Map(array_rule, predicate) => MAP_OP.apply(array_rule, predicate, context, root, rpath),
            Rule::Filter(array_rule, predicate) => FILTER_OP.apply(array_rule, predicate, context, root, rpath),
            Rule::Reduce(array_rule, reducer_rule, initial_rule) => 
                REDUCE_OP.apply(array_rule, reducer_rule, initial_rule, context, root, rpath),
            Rule::Merge(args) => MERGE_OP.apply(args, context, root, rpath),
            Rule::ArrayPredicate(op, array_rule, predicate) => 
                ARRAY_PREDICATE_OP.apply(array_rule, predicate, context, root, rpath, op),

            Rule::Compare(op, args) => COMPARE_OP.apply(args, context, root, rpath, op),
            Rule::Logic(op, args) => LOGIC_OP.apply(args, context, root, rpath, op),
            Rule::Arithmetic(op, args) => ARITHMETIC_OP.apply(args, context, root, rpath, op),
    
            Rule::If(args) => IF_OP.apply(args, context, root, rpath),
            Rule::Coalesce(args) => COALESCE_OP.apply(args, context, root, rpath),

            Rule::In(search, target) => IN_OP.apply(search, target, context, root, rpath),
            Rule::Cat(args) => CAT_OP.apply(args, context, root, rpath),
            Rule::Substr(string, start, length) => 
                SUBSTR_OP.apply(string, start, length.as_deref(), context, root, rpath),

            Rule::Missing(args) => MISSING_OP.apply(args, context, root, rpath),
            Rule::MissingSome(args) => MISSING_SOME_OP.apply(args, context, root, rpath),
            
            Rule::Throw(rule) => {
                let result = rule.apply(context, root, rpath)?;
                Err(Error::Custom(result.into_owned().to_string()))
            },
            Rule::Try(args) => TRY_OP.apply(args, context, root, rpath),
            Rule::Custom(name, args) => {
                let json_logic = JsonLogic::global();
                if let Some(op) = json_logic.get_operator(name) {
                    let mut evaluated_args = Vec::with_capacity(args.len());
                    for arg in args {
                        evaluated_args.push(arg.apply(context, root, rpath)?.into_owned());
                    }
                    op.apply(&evaluated_args, context, root, rpath)
                } else {
                    Err(Error::UnknownExpression(name.clone()))
                }
            }
        }
    }

    /// Optimize this rule in-place
    pub fn optimize_in_place(&mut self) -> Result<(), Error> {
        let optimized = std::mem::replace(self, Rule::Value(Value::Null)).optimize()?;
        *self = optimized;
        Ok(())
    }
}