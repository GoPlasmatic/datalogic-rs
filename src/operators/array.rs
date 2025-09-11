use serde_json::Value;

use std::collections::HashMap;

use crate::context::{index_key, key_key};
use crate::value_helpers::is_truthy;
use crate::{ContextStack, Evaluator, Operator, Result};

/// Merge operator - merges arrays
pub struct MergeOperator;

impl Operator for MergeOperator {
    fn evaluate(
        &self,
        args: &[Value],
        context: &mut ContextStack,
        evaluator: &dyn Evaluator,
    ) -> Result<Value> {
        let mut result = Vec::new();

        for arg in args {
            let value = evaluator.evaluate(arg, context)?;
            match value {
                Value::Array(arr) => result.extend(arr.iter().cloned()),
                v => result.push(v.clone()),
            }
        }

        Ok(Value::Array(result))
    }
}

/// Map operator - transforms array/object elements
pub struct MapOperator;

impl Operator for MapOperator {
    fn evaluate(
        &self,
        args: &[Value],
        context: &mut ContextStack,
        evaluator: &dyn Evaluator,
    ) -> Result<Value> {
        if args.len() < 2 {
            return Ok(Value::Array(vec![]));
        }

        let collection = evaluator.evaluate(&args[0], context)?;
        let logic = &args[1];

        match &collection {
            Value::Array(arr) => {
                let mut results = Vec::with_capacity(arr.len());

                for (index, item) in arr.iter().enumerate() {
                    let mut metadata = HashMap::with_capacity(1);
                    metadata.insert(index_key().clone(), Value::Number(index.into()));

                    context.push_with_metadata(item.clone(), metadata);
                    let result = evaluator.evaluate(logic, context)?;
                    results.push(result);
                    context.pop();
                }

                Ok(Value::Array(results))
            }
            Value::Object(obj) => {
                let mut results = Vec::with_capacity(obj.len());

                for (index, (key, value)) in obj.iter().enumerate() {
                    let mut metadata = HashMap::with_capacity(2);
                    metadata.insert(key_key().clone(), Value::String(key.clone()));
                    metadata.insert(index_key().clone(), Value::Number(index.into()));

                    context.push_with_metadata(value.clone(), metadata);
                    let result = evaluator.evaluate(logic, context)?;
                    results.push(result);
                    context.pop();
                }

                Ok(Value::Array(results))
            }
            _ => Ok(Value::Array(vec![])),
        }
    }
}

/// Filter operator - filters array/object elements
pub struct FilterOperator;

impl Operator for FilterOperator {
    fn evaluate(
        &self,
        args: &[Value],
        context: &mut ContextStack,
        evaluator: &dyn Evaluator,
    ) -> Result<Value> {
        if args.len() < 2 {
            return Ok(Value::Array(vec![]));
        }

        let collection = evaluator.evaluate(&args[0], context)?;
        let predicate = &args[1];

        match &collection {
            Value::Array(arr) => {
                // Fast path for constant predicates
                match predicate {
                    Value::Bool(false) => return Ok(Value::Array(vec![])),
                    Value::Bool(true) => return Ok(Value::Array(arr.clone())),
                    _ => {}
                }

                let mut results = Vec::new();

                for (index, item) in arr.iter().enumerate() {
                    let mut metadata = HashMap::with_capacity(1);
                    metadata.insert(index_key().clone(), Value::Number(index.into()));

                    context.push_with_metadata(item.clone(), metadata);
                    let keep = evaluator.evaluate(predicate, context)?;
                    context.pop();

                    if is_truthy(&keep) {
                        results.push(item.clone());
                    }
                }

                Ok(Value::Array(results))
            }
            Value::Object(obj) => {
                let mut result_obj = serde_json::Map::new();

                for (index, (key, value)) in obj.iter().enumerate() {
                    let mut metadata = HashMap::with_capacity(2);
                    metadata.insert(key_key().clone(), Value::String(key.clone()));
                    metadata.insert(index_key().clone(), Value::Number(index.into()));

                    context.push_with_metadata(value.clone(), metadata);
                    let keep = evaluator.evaluate(predicate, context)?;
                    context.pop();

                    if is_truthy(&keep) {
                        result_obj.insert(key.clone(), value.clone());
                    }
                }

                Ok(Value::Object(result_obj))
            }
            _ => Ok(Value::Array(vec![])),
        }
    }
}

/// Reduce operator
pub struct ReduceOperator;

impl Operator for ReduceOperator {
    fn evaluate(
        &self,
        args: &[Value],
        context: &mut ContextStack,
        evaluator: &dyn Evaluator,
    ) -> Result<Value> {
        if args.len() < 3 {
            return Ok(Value::Null);
        }

        let array = evaluator.evaluate(&args[0], context)?;
        let logic = &args[1];
        let initial = evaluator.evaluate(&args[2], context)?;

        match &array {
            Value::Array(arr) => {
                if arr.is_empty() {
                    return Ok(initial);
                }

                let mut accumulator = initial;

                for current in arr {
                    let mut frame_data = serde_json::Map::with_capacity(2);
                    frame_data.insert("current".to_string(), current.clone());
                    frame_data.insert("accumulator".to_string(), accumulator);

                    context.push(Value::Object(frame_data));
                    accumulator = evaluator.evaluate(logic, context)?;
                    context.pop();
                }

                Ok(accumulator)
            }
            _ => Ok(initial),
        }
    }
}

/// All operator - tests if all elements pass
pub struct AllOperator;

impl Operator for AllOperator {
    fn evaluate(
        &self,
        args: &[Value],
        context: &mut ContextStack,
        evaluator: &dyn Evaluator,
    ) -> Result<Value> {
        if args.len() < 2 {
            return Ok(Value::Bool(false));
        }

        let collection = evaluator.evaluate(&args[0], context)?;
        let predicate = &args[1];

        match &collection {
            Value::Array(arr) if !arr.is_empty() => {
                // Fast path for constant predicates
                match predicate {
                    Value::Bool(false) => return Ok(Value::Bool(false)),
                    Value::Bool(true) => return Ok(Value::Bool(true)),
                    _ => {}
                }

                for (index, item) in arr.iter().enumerate() {
                    let mut metadata = HashMap::with_capacity(1);
                    metadata.insert(index_key().clone(), Value::Number(index.into()));

                    context.push_with_metadata(item.clone(), metadata);
                    let result = evaluator.evaluate(predicate, context)?;
                    context.pop();

                    if !is_truthy(&result) {
                        return Ok(Value::Bool(false));
                    }
                }
                Ok(Value::Bool(true))
            }
            _ => Ok(Value::Bool(false)),
        }
    }
}

/// Some operator - tests if any element passes
pub struct SomeOperator;

impl Operator for SomeOperator {
    fn evaluate(
        &self,
        args: &[Value],
        context: &mut ContextStack,
        evaluator: &dyn Evaluator,
    ) -> Result<Value> {
        if args.len() < 2 {
            return Ok(Value::Bool(false));
        }

        let collection = evaluator.evaluate(&args[0], context)?;
        let predicate = &args[1];

        match &collection {
            Value::Array(arr) => {
                // Fast path for constant predicates
                match predicate {
                    Value::Bool(false) => return Ok(Value::Bool(false)),
                    Value::Bool(true) => return Ok(Value::Bool(!arr.is_empty())),
                    _ => {}
                }

                for (index, item) in arr.iter().enumerate() {
                    let mut metadata = HashMap::with_capacity(1);
                    metadata.insert(index_key().clone(), Value::Number(index.into()));

                    context.push_with_metadata(item.clone(), metadata);
                    let result = evaluator.evaluate(predicate, context)?;
                    context.pop();

                    if is_truthy(&result) {
                        return Ok(Value::Bool(true));
                    }
                }
                Ok(Value::Bool(false))
            }
            _ => Ok(Value::Bool(false)),
        }
    }
}

/// None operator - tests if no elements pass
pub struct NoneOperator;

impl Operator for NoneOperator {
    fn evaluate(
        &self,
        args: &[Value],
        context: &mut ContextStack,
        evaluator: &dyn Evaluator,
    ) -> Result<Value> {
        if args.len() < 2 {
            return Ok(Value::Bool(true));
        }

        let collection = evaluator.evaluate(&args[0], context)?;
        let predicate = &args[1];

        match &collection {
            Value::Array(arr) => {
                // Fast path for constant predicates
                match predicate {
                    Value::Bool(false) => return Ok(Value::Bool(true)),
                    Value::Bool(true) => return Ok(Value::Bool(arr.is_empty())),
                    _ => {}
                }

                for (index, item) in arr.iter().enumerate() {
                    let mut metadata = HashMap::with_capacity(1);
                    metadata.insert(index_key().clone(), Value::Number(index.into()));

                    context.push_with_metadata(item.clone(), metadata);
                    let result = evaluator.evaluate(predicate, context)?;
                    context.pop();

                    if is_truthy(&result) {
                        return Ok(Value::Bool(false));
                    }
                }
                Ok(Value::Bool(true))
            }
            _ => Ok(Value::Bool(true)),
        }
    }
}
