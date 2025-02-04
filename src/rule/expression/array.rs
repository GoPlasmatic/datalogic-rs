use serde_json::Value;
use crate::{rule::Rule, OpType};

use super::{coercion::ValueCoercion, EvalResult};

pub fn evaluate_map(args: &[Value], mapper: &Rule) -> EvalResult {
    if let Some(Value::Array(items)) = args.first() {
        let mut mapped = Vec::with_capacity(items.len());
        
        for item in items {
            mapped.push(mapper.apply(item).unwrap_or(Value::Null));
        }
        
        Ok(Value::Array(mapped))
    } else {
        Ok(Value::Array(Vec::new()))
    }
}

pub fn evaluate_filter(args: &[Value], predicate: &Rule) -> EvalResult {
    if args.is_empty() {
        return Ok(Value::Array(vec![]));
    }

    let array = match &args[0] {
        Value::Array(items) => items,
        _ => return Ok(Value::Array(vec![])),
    };

    let filtered = array
        .iter()
        .filter(|item| {
            predicate
                .apply(item)
                .map(|v| v.coerce_to_bool())
                .unwrap_or(false)
        })
        .cloned()
        .collect::<Vec<_>>();

    Ok(Value::Array(filtered))
}

pub fn evaluate_array_op(op: &OpType, args: &[Value], predicate: &Rule) -> EvalResult {
    let items = match args.first() {
        Some(Value::Array(items)) if !items.is_empty() => items,
        _ => return Ok(Value::Bool(*op == OpType::None))
    };

    let result = match op {
        OpType::All => items.iter().all(|item| predicate
            .apply(item)
            .map(|v| v.coerce_to_bool())
            .unwrap_or(false)),
        OpType::Some => items.iter().any(|item| predicate
            .apply(item)
            .map(|v| v.coerce_to_bool())
            .unwrap_or(false)),
        OpType::None => !items.iter().any(|item| predicate
            .apply(item)
            .map(|v| v.coerce_to_bool())
            .unwrap_or(false)),
        _ => false,
    };
    
    Ok(Value::Bool(result))
}

pub fn evaluate_reduce(args: &[Value], reducer: &Rule) -> EvalResult {
    if let [args@.., initial] = args {
        let items = match &args[0] {
            Value::Array(items) => items,
            _ => return Ok(initial.clone()),
        };

        let mut result = initial.clone();
        let mut data = {
            let mut map = serde_json::Map::with_capacity(2);
            map.insert("current".to_string(), Value::Null);
            map.insert("accumulator".to_string(), Value::Null);
            Value::Object(map)
        };

        for item in items {
            if let Value::Object(ref mut map) = data {
                if let Some(v) = map.get_mut("current") {
                    *v = item.clone();
                }
                if let Some(v) = map.get_mut("accumulator") {
                    *v = result.clone();
                }
                result = reducer.apply(&data)?;
            }
        }
        Ok(result)
    } else {
        Ok(Value::Null)
    }
}

pub fn evaluate_merge(args: &[Value]) -> EvalResult {
    let total_len = args.iter()
        .map(|arg| match arg {
            Value::Array(arr) => arr.len(),
            _ => 1
        })
        .sum();

    let mut result = Vec::with_capacity(total_len);
    for arg in args {
        match arg {
            Value::Array(items) => result.extend_from_slice(items),
            value => result.push(value.clone()),
        }
    }

    Ok(Value::Array(result))
}