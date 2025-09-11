use serde_json::Value;
use std::borrow::Cow;
use std::collections::HashMap;

use crate::value_helpers::is_truthy;
use crate::{ContextStack, Evaluator, Operator, Result};

/// Merge operator - merges arrays
pub struct MergeOperator;

impl Operator for MergeOperator {
    fn evaluate<'a>(
        &self,
        args: &[Cow<'a, Value>],
        context: &mut ContextStack<'a>,
        evaluator: &dyn Evaluator,
    ) -> Result<Cow<'a, Value>> {
        let mut result = Vec::new();

        for arg in args {
            let value = evaluator.evaluate(arg, context)?;
            match value.as_ref() {
                Value::Array(arr) => result.extend(arr.iter().cloned()),
                v => result.push(v.clone()),
            }
        }

        Ok(Cow::Owned(Value::Array(result)))
    }
}

/// Map operator - transforms array/object elements
pub struct MapOperator;

impl Operator for MapOperator {
    fn evaluate<'a>(
        &self,
        args: &[Cow<'a, Value>],
        context: &mut ContextStack<'a>,
        evaluator: &dyn Evaluator,
    ) -> Result<Cow<'a, Value>> {
        if args.len() < 2 {
            return Ok(Cow::Owned(Value::Array(vec![])));
        }

        let collection = evaluator.evaluate(&args[0], context)?;
        let logic = &args[1];

        match collection.as_ref() {
            Value::Array(arr) => {
                let mut results = Vec::with_capacity(arr.len());

                for (index, item) in arr.iter().enumerate() {
                    let mut metadata = HashMap::new();
                    metadata.insert("index".to_string(), Cow::Owned(Value::Number(index.into())));

                    context.push_with_metadata(Cow::Owned(item.clone()), metadata);
                    let result = evaluator.evaluate(logic, context)?;
                    results.push(result.into_owned());
                    context.pop();
                }

                Ok(Cow::Owned(Value::Array(results)))
            }
            Value::Object(obj) => {
                let mut results = Vec::with_capacity(obj.len());

                for (key, value) in obj.iter() {
                    let mut metadata = HashMap::new();
                    metadata.insert("key".to_string(), Cow::Owned(Value::String(key.clone())));
                    metadata.insert(
                        "index".to_string(),
                        Cow::Owned(Value::Number(results.len().into())),
                    );

                    context.push_with_metadata(Cow::Owned(value.clone()), metadata);
                    let result = evaluator.evaluate(logic, context)?;
                    results.push(result.into_owned());
                    context.pop();
                }

                Ok(Cow::Owned(Value::Array(results)))
            }
            _ => Ok(Cow::Owned(Value::Array(vec![]))),
        }
    }
}

/// Filter operator - filters array/object elements
pub struct FilterOperator;

impl Operator for FilterOperator {
    fn evaluate<'a>(
        &self,
        args: &[Cow<'a, Value>],
        context: &mut ContextStack<'a>,
        evaluator: &dyn Evaluator,
    ) -> Result<Cow<'a, Value>> {
        if args.len() < 2 {
            return Ok(Cow::Owned(Value::Array(vec![])));
        }

        let collection = evaluator.evaluate(&args[0], context)?;
        let predicate = &args[1];

        match collection.as_ref() {
            Value::Array(arr) => {
                let mut results = Vec::new();

                for (index, item) in arr.iter().enumerate() {
                    let mut metadata = HashMap::new();
                    metadata.insert("index".to_string(), Cow::Owned(Value::Number(index.into())));

                    context.push_with_metadata(Cow::Owned(item.clone()), metadata);
                    let keep = evaluator.evaluate(predicate, context)?;
                    context.pop();

                    if is_truthy(keep.as_ref()) {
                        results.push(item.clone());
                    }
                }

                Ok(Cow::Owned(Value::Array(results)))
            }
            Value::Object(obj) => {
                let mut result_obj = serde_json::Map::new();

                for (index, (key, value)) in obj.iter().enumerate() {
                    let mut metadata = HashMap::new();
                    metadata.insert("key".to_string(), Cow::Owned(Value::String(key.clone())));
                    metadata.insert("index".to_string(), Cow::Owned(Value::Number(index.into())));

                    context.push_with_metadata(Cow::Owned(value.clone()), metadata);
                    let keep = evaluator.evaluate(predicate, context)?;
                    context.pop();

                    if is_truthy(keep.as_ref()) {
                        result_obj.insert(key.clone(), value.clone());
                    }
                }

                Ok(Cow::Owned(Value::Object(result_obj)))
            }
            _ => Ok(Cow::Owned(Value::Array(vec![]))),
        }
    }
}

/// Reduce operator
pub struct ReduceOperator;

impl Operator for ReduceOperator {
    fn evaluate<'a>(
        &self,
        args: &[Cow<'a, Value>],
        context: &mut ContextStack<'a>,
        evaluator: &dyn Evaluator,
    ) -> Result<Cow<'a, Value>> {
        if args.len() < 3 {
            return Ok(Cow::Owned(Value::Null));
        }

        let array = evaluator.evaluate(&args[0], context)?;
        let logic = &args[1];
        let initial = evaluator.evaluate(&args[2], context)?;

        match array.as_ref() {
            Value::Array(arr) => {
                let mut accumulator = initial.into_owned();

                for current in arr {
                    let mut frame_data = serde_json::Map::new();
                    frame_data.insert("current".to_string(), current.clone());
                    frame_data.insert("accumulator".to_string(), accumulator.clone());

                    context.push(Cow::Owned(Value::Object(frame_data)));
                    accumulator = evaluator.evaluate(logic, context)?.into_owned();
                    context.pop();
                }

                Ok(Cow::Owned(accumulator))
            }
            _ => Ok(initial),
        }
    }
}

/// All operator - tests if all elements pass
pub struct AllOperator;

impl Operator for AllOperator {
    fn evaluate<'a>(
        &self,
        args: &[Cow<'a, Value>],
        context: &mut ContextStack<'a>,
        evaluator: &dyn Evaluator,
    ) -> Result<Cow<'a, Value>> {
        if args.len() < 2 {
            return Ok(Cow::Owned(Value::Bool(false)));
        }

        let collection = evaluator.evaluate(&args[0], context)?;
        let predicate = &args[1];

        match collection.as_ref() {
            Value::Array(arr) if !arr.is_empty() => {
                for (index, item) in arr.iter().enumerate() {
                    let mut metadata = HashMap::new();
                    metadata.insert("index".to_string(), Cow::Owned(Value::Number(index.into())));

                    context.push_with_metadata(Cow::Owned(item.clone()), metadata);
                    let result = evaluator.evaluate(predicate, context)?;
                    context.pop();

                    if !is_truthy(result.as_ref()) {
                        return Ok(Cow::Owned(Value::Bool(false)));
                    }
                }
                Ok(Cow::Owned(Value::Bool(true)))
            }
            _ => Ok(Cow::Owned(Value::Bool(false))),
        }
    }
}

/// Some operator - tests if any element passes
pub struct SomeOperator;

impl Operator for SomeOperator {
    fn evaluate<'a>(
        &self,
        args: &[Cow<'a, Value>],
        context: &mut ContextStack<'a>,
        evaluator: &dyn Evaluator,
    ) -> Result<Cow<'a, Value>> {
        if args.len() < 2 {
            return Ok(Cow::Owned(Value::Bool(false)));
        }

        let collection = evaluator.evaluate(&args[0], context)?;
        let predicate = &args[1];

        match collection.as_ref() {
            Value::Array(arr) => {
                for (index, item) in arr.iter().enumerate() {
                    let mut metadata = HashMap::new();
                    metadata.insert("index".to_string(), Cow::Owned(Value::Number(index.into())));

                    context.push_with_metadata(Cow::Owned(item.clone()), metadata);
                    let result = evaluator.evaluate(predicate, context)?;
                    context.pop();

                    if is_truthy(result.as_ref()) {
                        return Ok(Cow::Owned(Value::Bool(true)));
                    }
                }
                Ok(Cow::Owned(Value::Bool(false)))
            }
            _ => Ok(Cow::Owned(Value::Bool(false))),
        }
    }
}

/// None operator - tests if no elements pass
pub struct NoneOperator;

impl Operator for NoneOperator {
    fn evaluate<'a>(
        &self,
        args: &[Cow<'a, Value>],
        context: &mut ContextStack<'a>,
        evaluator: &dyn Evaluator,
    ) -> Result<Cow<'a, Value>> {
        if args.len() < 2 {
            return Ok(Cow::Owned(Value::Bool(true)));
        }

        let collection = evaluator.evaluate(&args[0], context)?;
        let predicate = &args[1];

        match collection.as_ref() {
            Value::Array(arr) => {
                for (index, item) in arr.iter().enumerate() {
                    let mut metadata = HashMap::new();
                    metadata.insert("index".to_string(), Cow::Owned(Value::Number(index.into())));

                    context.push_with_metadata(Cow::Owned(item.clone()), metadata);
                    let result = evaluator.evaluate(predicate, context)?;
                    context.pop();

                    if is_truthy(result.as_ref()) {
                        return Ok(Cow::Owned(Value::Bool(false)));
                    }
                }
                Ok(Cow::Owned(Value::Bool(true)))
            }
            _ => Ok(Cow::Owned(Value::Bool(true))),
        }
    }
}
