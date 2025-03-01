use serde_json::Value;
use crate::Error;
use super::{Rule, ValueCoercion, StaticEvaluable};
use std::borrow::Cow;

pub struct MapOperator;
pub struct FilterOperator;
pub struct ReduceOperator;
pub struct MergeOperator;

impl MapOperator {
    pub fn apply<'a>(&self, array_rule: &Rule, mapper: &Rule, context: &Value, root: &Value, path: &str) -> Result<Cow<'a, Value>, Error> {
        if let Rule::Value(arr_val) = array_rule {
            if arr_val.is_null() {
                return Err(Error::Custom("Invalid Arguments".into()));
            }
        }
        if let Rule::Value(mapper_val) = mapper {
            if mapper_val.is_null() {
                return Err(Error::Custom("Invalid Arguments".into()));
            }
        }

        let array_value = array_rule.apply(context, root, path)?;
        match array_value.as_ref() {
            Value::Array(arr) => {
                let mut results = Vec::with_capacity(arr.len());
                for (index, item) in arr.iter().enumerate() {
                    let path = &format!("{}[{}]", path, index);
                    results.push(mapper.apply(item, root, path)?.into_owned());
                }
                Ok(Cow::Owned(Value::Array(results)))
            },
            _ => Ok(Cow::Owned(Value::Array(Vec::new())))
        }
    }
}

impl StaticEvaluable for MapOperator {
    fn is_static(&self, rule: &Rule) -> bool {
        if let Rule::Map(array_rule, mapper) = rule {
            array_rule.is_static() && mapper.is_static()
        } else {
            false
        }
    }
}

impl FilterOperator {
    pub fn apply<'a>(&self, array_rule: &Rule, predicate: &Rule, context: &Value, root: &Value, path: &str) -> Result<Cow<'a, Value>, Error> {
        if let Rule::Value(arr_val) = array_rule {
            if arr_val.is_null() {
                return Err(Error::Custom("Invalid Arguments".into()));
            }
        }
        if let Rule::Value(predicate_val) = predicate {
            if predicate_val.is_null() {
                return Err(Error::Custom("Invalid Arguments".into()));
            }
        }

        let array_value = array_rule.apply(context, root, path)?;
        match array_value.as_ref() {
            Value::Array(arr) => {
                let mut results = Vec::with_capacity(arr.len());
                for (index, item) in arr.iter().enumerate() {
                    let path = &format!("{}[{}]", path, index);
                    let result = predicate.apply(item, root, path)?;
                    if result.coerce_to_bool() {
                        results.push(item.to_owned());
                    }
                }
                
                Ok(Cow::Owned(Value::Array(results)))
            },
            _ => Ok(Cow::Owned(Value::Array(Vec::with_capacity(0))))
        }
    }
}

impl StaticEvaluable for FilterOperator {
    fn is_static(&self, rule: &Rule) -> bool {
        if let Rule::Filter(array_rule, predicate) = rule {
            array_rule.is_static() && predicate.is_static()
        } else {
            false
        }
    }
}

impl ReduceOperator {
    pub fn apply<'a>(&self, array_rule: &Rule, reducer_rule: &Rule, initial_rule: &'a Rule, context: &'a Value, root: &'a Value, path: &str) -> Result<Cow<'a, Value>, Error> {
        let array_value = array_rule.apply(context, root, path)?;

        match array_value.as_ref() {
            Value::Array(arr) if arr.is_empty() => {
                initial_rule.apply(context, root, path)
            },
            Value::Array(arr) => {
                let mut accumulator = initial_rule.apply(context, root, path)?.into_owned();

                for (index, current) in arr.iter().enumerate() {
                    let mut context = serde_json::Map::with_capacity(2);
                    context.insert("current".to_string(), current.to_owned());
                    context.insert("accumulator".to_string(), accumulator);
                    
                    let path = &format!("{}[{}]", path, index);
                    accumulator = reducer_rule.apply(&Value::Object(context), root, path)?.into_owned();
                }
                
                Ok(Cow::Owned(accumulator))
            },
            _ => initial_rule.apply(context, root, path),
        }
    }
}

impl StaticEvaluable for ReduceOperator {
    fn is_static(&self, rule: &Rule) -> bool {
        if let Rule::Reduce(array_rule, reducer, initial) = rule {
            array_rule.is_static() && reducer.is_static() && initial.is_static()
        } else {
            false
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum ArrayPredicateType {
    All,
    Some,
    None,
    Invalid
}

pub struct ArrayPredicateOperator;

impl ArrayPredicateOperator {
    pub fn apply<'a>(&self, array_rule: &Rule, predicate: &Rule, context: &Value, root: &Value, path: &str, op_type: &ArrayPredicateType) 
        -> Result<Cow<'a, Value>, Error> 
    {
        if *op_type == ArrayPredicateType::Invalid {
            return Err(Error::Custom("Invalid Arguments".into()));
        }

        let array_value = array_rule.apply(context, root, path)?;

        match array_value.as_ref() {
            Value::Array(arr) => {
                let result = match op_type {
                    ArrayPredicateType::All => {
                        if arr.is_empty() {
                            false
                        } else {
                            arr.iter().enumerate()
                                .all(|(index, item)| matches!(predicate.apply(item, root, &format!("{}[{}]", path, index)), Ok(v) if v.coerce_to_bool()))
                        }
                    },
                    ArrayPredicateType::Some => {
                        if arr.is_empty() {
                            false
                        } else {
                            arr.iter().enumerate()
                                .any(|(index, item)| matches!(predicate.apply(item, root, &format!("{}[{}]", path, index)), Ok(v) if v.coerce_to_bool()))
                        }
                    },
                    ArrayPredicateType::None => {
                        if arr.is_empty() {
                            true
                        } else {
                            !arr.iter().enumerate()
                                .any(|(index, item)| matches!(predicate.apply(item, root, &format!("{}[{}]", path, index)), Ok(v) if v.coerce_to_bool()))
                        }
                    },
                    _ => unreachable!()
                };
                Ok(Cow::Owned(Value::Bool(result)))
            },
            _ => Err(Error::Custom("Invalid Arguments".into()))
        }
    }
}

impl StaticEvaluable for ArrayPredicateOperator {
    fn is_static(&self, rule: &Rule) -> bool {
        if let Rule::ArrayPredicate(_, array_rule, predicate) = rule {
            array_rule.is_static() && predicate.is_static()
        } else {
            false
        }
    }
}

impl MergeOperator {
    pub fn apply<'a>(&self, args: &[Rule], context: &Value, root: &Value, path: &str) -> Result<Cow<'a, Value>, Error> {
        if args.is_empty() {
            return Ok(Cow::Owned(Value::Array(Vec::new())));
        }
        
        let capacity = args.len() * 2;
        let mut merged = Vec::with_capacity(capacity);
        
        for arg in args {
            match arg.apply(context, root, path)? {
                Cow::Owned(Value::Array(arr)) => merged.extend(arr),
                Cow::Borrowed(Value::Array(arr)) => merged.extend(arr.iter().cloned()),
                value => merged.push(value.into_owned()),
            }
        }
        
        Ok(Cow::Owned(Value::Array(merged)))
    }
}

impl StaticEvaluable for MergeOperator {
    fn is_static(&self, rule: &Rule) -> bool {
        if let Rule::Merge(args) = rule {
            args.iter().all(|r| r.is_static())
        } else {
            false
        }
    }
}