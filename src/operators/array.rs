use serde_json::Value;

use std::cmp::Ordering;
use std::collections::HashMap;

use super::helpers::is_truthy;
use crate::constants::INVALID_ARGS;
use crate::opcode::OpCode;
use crate::trace::TraceCollector;
use crate::{CompiledNode, ContextStack, DataLogic, Error, Result};

/// Check if a compiled node is loop-invariant (doesn't depend on the current iteration context).
/// Used by filter/quantifier fast paths to detect values that can be evaluated once before the loop.
#[inline]
fn is_filter_invariant(node: &CompiledNode) -> bool {
    match node {
        CompiledNode::Value { .. } => true,
        CompiledNode::CompiledVar { scope_level, .. } => *scope_level > 0,
        _ => false,
    }
}

/// Try to extract filter fast-path components from a comparison pair.
/// Returns (field_segments, invariant_node) if `a` is a simple scope_level=0 field var
/// and `b` is loop-invariant (literal value or parent scope var).
#[inline]
fn try_extract_filter_field_cmp<'a>(
    a: &'a CompiledNode,
    b: &'a CompiledNode,
) -> Option<(&'a [crate::compiled::PathSegment], &'a CompiledNode)> {
    if let CompiledNode::CompiledVar {
        scope_level: 0,
        segments,
        reduce_hint: crate::compiled::ReduceHint::None,
        metadata_hint: crate::compiled::MetadataHint::None,
        default_value: None,
    } = a
        && !segments.is_empty()
        && is_filter_invariant(b)
    {
        return Some((segments, b));
    }
    None
}

/// Represents a detected fast-path predicate pattern for quantifier/filter operators.
/// Avoids per-item context push/pop and evaluate_node dispatch.
enum FastPredicate<'a> {
    /// Compare whole item (val([])) against a literal using strict equality
    WholeItemStrictEq { literal: &'a Value, negate: bool },
    /// Compare a field against a literal using strict equality
    FieldStrictEq {
        segments: &'a [crate::compiled::PathSegment],
        literal: &'a Value,
        negate: bool,
    },
    /// Compare whole item (val([])) against a numeric literal using ordered comparison
    WholeItemNumericCmp {
        literal_f: f64,
        opcode: OpCode,
        var_is_lhs: bool,
    },
    /// Compare a field against a numeric literal using ordered comparison
    FieldNumericCmp {
        segments: &'a [crate::compiled::PathSegment],
        literal_f: f64,
        opcode: OpCode,
        var_is_lhs: bool,
    },
    /// Compare whole item against a numeric literal using loose equality (==/!=)
    WholeItemLooseNumericEq { literal_f: f64, negate: bool },
    /// Compare a field against a numeric literal using loose equality (==/!=)
    FieldLooseNumericEq {
        segments: &'a [crate::compiled::PathSegment],
        literal_f: f64,
        negate: bool,
    },
}

impl<'a> FastPredicate<'a> {
    /// Try to detect a fast predicate pattern from a compiled predicate node.
    fn try_detect(predicate: &'a CompiledNode) -> Option<Self> {
        if let CompiledNode::BuiltinOperator {
            opcode,
            args: pred_args,
        } = predicate
            && pred_args.len() == 2
        {
            // Try both orderings: (var, literal) and (literal, var)
            for (var_idx, lit_idx, var_is_lhs) in [(0, 1, true), (1, 0, false)] {
                if let CompiledNode::CompiledVar {
                    scope_level: 0,
                    segments,
                    reduce_hint: crate::compiled::ReduceHint::None,
                    metadata_hint: crate::compiled::MetadataHint::None,
                    default_value: None,
                } = &pred_args[var_idx]
                    && let CompiledNode::Value { value: literal } = &pred_args[lit_idx]
                {
                    let is_whole_item = segments.is_empty();

                    match opcode {
                        OpCode::StrictEquals | OpCode::StrictNotEquals => {
                            let negate = matches!(opcode, OpCode::StrictNotEquals);
                            if is_whole_item {
                                return Some(FastPredicate::WholeItemStrictEq { literal, negate });
                            } else {
                                return Some(FastPredicate::FieldStrictEq {
                                    segments,
                                    literal,
                                    negate,
                                });
                            }
                        }
                        OpCode::Equals | OpCode::NotEquals => {
                            // For loose equality with numeric literals, we can use a fast
                            // numeric comparison (loose == is same as strict for numbers)
                            if let Some(lit_f) = literal.as_f64() {
                                let negate = matches!(opcode, OpCode::NotEquals);
                                if is_whole_item {
                                    return Some(FastPredicate::WholeItemLooseNumericEq {
                                        literal_f: lit_f,
                                        negate,
                                    });
                                } else {
                                    return Some(FastPredicate::FieldLooseNumericEq {
                                        segments,
                                        literal_f: lit_f,
                                        negate,
                                    });
                                }
                            }
                        }
                        OpCode::GreaterThan
                        | OpCode::GreaterThanEqual
                        | OpCode::LessThan
                        | OpCode::LessThanEqual => {
                            if let Some(lit_f) = literal.as_f64() {
                                if is_whole_item {
                                    return Some(FastPredicate::WholeItemNumericCmp {
                                        literal_f: lit_f,
                                        opcode: *opcode,
                                        var_is_lhs,
                                    });
                                } else {
                                    return Some(FastPredicate::FieldNumericCmp {
                                        segments,
                                        literal_f: lit_f,
                                        opcode: *opcode,
                                        var_is_lhs,
                                    });
                                }
                            }
                        }
                        _ => {}
                    }
                }
            }
        }
        None
    }

    /// Evaluate this predicate against a single item.
    #[inline(always)]
    fn evaluate(&self, item: &Value) -> bool {
        match self {
            FastPredicate::WholeItemStrictEq { literal, negate } => {
                let matches = item == *literal;
                if *negate { !matches } else { matches }
            }
            FastPredicate::FieldStrictEq {
                segments,
                literal,
                negate,
            } => {
                let matches =
                    super::variable::try_traverse_segments(item, segments) == Some(*literal);
                if *negate { !matches } else { matches }
            }
            FastPredicate::WholeItemNumericCmp {
                literal_f,
                opcode,
                var_is_lhs,
            } => {
                if let Some(item_f) = item.as_f64() {
                    let (lhs, rhs) = if *var_is_lhs {
                        (item_f, *literal_f)
                    } else {
                        (*literal_f, item_f)
                    };
                    inline_numeric_cmp(lhs, rhs, *opcode)
                } else {
                    false
                }
            }
            FastPredicate::FieldNumericCmp {
                segments,
                literal_f,
                opcode,
                var_is_lhs,
            } => {
                if let Some(field_val) = super::variable::try_traverse_segments(item, segments) {
                    if let Some(field_f) = field_val.as_f64() {
                        let (lhs, rhs) = if *var_is_lhs {
                            (field_f, *literal_f)
                        } else {
                            (*literal_f, field_f)
                        };
                        inline_numeric_cmp(lhs, rhs, *opcode)
                    } else {
                        false
                    }
                } else {
                    false
                }
            }
            FastPredicate::WholeItemLooseNumericEq { literal_f, negate } => {
                let matches = if let Some(item_f) = item.as_f64() {
                    item_f == *literal_f
                } else {
                    false
                };
                if *negate { !matches } else { matches }
            }
            FastPredicate::FieldLooseNumericEq {
                segments,
                literal_f,
                negate,
            } => {
                let matches = if let Some(field_val) =
                    super::variable::try_traverse_segments(item, segments)
                {
                    if let Some(field_f) = field_val.as_f64() {
                        field_f == *literal_f
                    } else {
                        false
                    }
                } else {
                    false
                };
                if *negate { !matches } else { matches }
            }
        }
    }
}

/// Inline numeric comparison for fast predicate evaluation.
#[inline(always)]
fn inline_numeric_cmp(lhs: f64, rhs: f64, opcode: OpCode) -> bool {
    match opcode {
        OpCode::GreaterThan => lhs > rhs,
        OpCode::GreaterThanEqual => lhs >= rhs,
        OpCode::LessThan => lhs < rhs,
        OpCode::LessThanEqual => lhs <= rhs,
        _ => false,
    }
}

/// Try to execute a reduce fast path for simple arithmetic accumulation patterns.
/// Detects {op: [val("current"), val("accumulator")]} or {op: [val("accumulator"), val("current")]}.
/// Also handles field access on current: {op: [val("accumulator"), val("current", "field")]}.
fn try_reduce_fast_path(
    arr: &[Value],
    initial: &Value,
    body_args: &[CompiledNode],
    opcode: OpCode,
) -> Option<Value> {
    use crate::compiled::ReduceHint;

    // Identify which arg is current and which is accumulator
    let (current_arg, _acc_arg) = match (&body_args[0], &body_args[1]) {
        (
            CompiledNode::CompiledVar {
                reduce_hint: hint0, ..
            },
            CompiledNode::CompiledVar {
                reduce_hint: hint1, ..
            },
        ) => match (hint0, hint1) {
            (
                ReduceHint::Current | ReduceHint::CurrentPath,
                ReduceHint::Accumulator | ReduceHint::AccumulatorPath,
            ) => (&body_args[0], &body_args[1]),
            (
                ReduceHint::Accumulator | ReduceHint::AccumulatorPath,
                ReduceHint::Current | ReduceHint::CurrentPath,
            ) => (&body_args[1], &body_args[0]),
            _ => return None,
        },
        _ => return None,
    };

    // Extract field segments if current has a path (e.g., val("current", "qty"))
    let current_segments = if let CompiledNode::CompiledVar {
        segments,
        reduce_hint,
        ..
    } = current_arg
    {
        match reduce_hint {
            ReduceHint::Current => &[][..], // Direct value access
            ReduceHint::CurrentPath => {
                if segments.len() >= 2 {
                    &segments[1..]
                } else {
                    return None;
                }
            }
            _ => return None,
        }
    } else {
        return None;
    };

    // Try integer fast path first
    let mut acc_i = initial.as_i64();
    if acc_i.is_some() {
        let mut all_int = true;
        for item in arr {
            let current_val = if current_segments.is_empty() {
                item
            } else {
                super::variable::try_traverse_segments(item, current_segments)?
            };
            if let Some(cur_i) = current_val.as_i64() {
                let a = acc_i.unwrap();
                acc_i = Some(match opcode {
                    OpCode::Add => a.wrapping_add(cur_i),
                    OpCode::Multiply => a.wrapping_mul(cur_i),
                    OpCode::Subtract => a.wrapping_sub(cur_i),
                    _ => return None,
                });
            } else {
                all_int = false;
                break;
            }
        }
        if all_int {
            return acc_i.map(Value::from);
        }
    }

    // Fall back to f64 path
    let mut acc_f = initial.as_f64()?;
    for item in arr {
        let current_val = if current_segments.is_empty() {
            item
        } else {
            super::variable::try_traverse_segments(item, current_segments)?
        };
        let cur_f = current_val.as_f64()?;
        acc_f = match opcode {
            OpCode::Add => acc_f + cur_f,
            OpCode::Multiply => acc_f * cur_f,
            OpCode::Subtract => acc_f - cur_f,
            _ => return None,
        };
    }
    Some(Value::from(acc_f))
}

/// The `merge` operator - combines multiple arrays into one.
///
/// # Syntax
/// ```json
/// {"merge": [array1, array2, ...]}
/// ```
///
/// # Arguments
/// Any number of arrays or values to merge together.
///
/// # Behavior
/// - Arrays are flattened one level (elements are extracted)
/// - Non-array values are added as-is
/// - `null` values are filtered out from the result
///
/// # Example
/// ```json
/// {"merge": [[1, 2], [3, 4], 5]}
/// ```
/// Returns: `[1, 2, 3, 4, 5]`
///
/// # Example with nulls
/// ```json
/// {"merge": [[1, null, 2], [3]]}
/// ```
/// Returns: `[1, 2, 3]` (nulls filtered)
#[inline]
pub fn evaluate_merge(
    args: &[CompiledNode],
    context: &mut ContextStack,
    engine: &DataLogic,
) -> Result<Value> {
    let mut result = Vec::new();

    for arg in args {
        let value = engine.evaluate_node(arg, context)?;
        match value {
            Value::Array(arr) => {
                // Filter out null values when extending
                result.extend(arr.into_iter().filter(|v| !v.is_null()))
            }
            Value::Null => {
                // Skip null values entirely
            }
            v => result.push(v),
        }
    }

    Ok(Value::Array(result))
}

/// The `map` operator - transforms each element in an array or object.
///
/// # Syntax
/// ```json
/// {"map": [collection, transformation]}
/// ```
///
/// # Arguments
/// 1. An array or object to iterate over
/// 2. A transformation logic to apply to each element
///
/// # Context
/// During iteration, the current item becomes the context, and metadata is available:
/// - `{"var": ""}` or `{"var": "."}` - current item value
/// - `{"var": "index"}` - current index (arrays) or key (objects)
/// - `{"var": "key"}` - current key (objects only)
///
/// # Example with Array
/// ```json
/// {
///   "map": [
///     [{"name": "Alice", "age": 30}, {"name": "Bob", "age": 25}],
///     {"var": "name"}
///   ]
/// }
/// ```
/// Returns: `["Alice", "Bob"]`
///
/// # Example with Object
/// ```json
/// {
///   "map": [
///     {"a": 1, "b": 2, "c": 3},
///     {"*": [{"var": ""}, 2]}
///   ]
/// }
/// ```
/// Returns: `[2, 4, 6]`
#[inline]
pub fn evaluate_map(
    args: &[CompiledNode],
    context: &mut ContextStack,
    engine: &DataLogic,
) -> Result<Value> {
    if args.len() != 2 {
        return Err(Error::InvalidArguments(INVALID_ARGS.into()));
    }

    let collection = engine.evaluate_node(&args[0], context)?;
    let logic = &args[1];

    match collection {
        Value::Array(arr) => {
            // Fast path: if the map body is a simple var/val access,
            // handle it directly without pushing items into context.
            if let CompiledNode::CompiledVar {
                scope_level: 0,
                segments,
                reduce_hint: crate::compiled::ReduceHint::None,
                metadata_hint: crate::compiled::MetadataHint::None,
                default_value: None,
            } = logic
            {
                if segments.is_empty() {
                    // Identity map: val([]) returns each item as-is
                    return Ok(Value::Array(arr));
                }
                // Field extraction: val("field") extracts a field from each item
                let mut results = Vec::with_capacity(arr.len());
                for item in arr.iter() {
                    let val = super::variable::try_traverse_segments(item, segments)
                        .cloned()
                        .unwrap_or(Value::Null);
                    results.push(val);
                }
                return Ok(Value::Array(results));
            }

            // Fast path: arithmetic op on whole item with literal
            // e.g., {"*": [{"val": []}, 2]}
            if let CompiledNode::BuiltinOperator {
                opcode,
                args: body_args,
            } = logic
                && body_args.len() == 2
                && matches!(
                    opcode,
                    OpCode::Add
                        | OpCode::Subtract
                        | OpCode::Multiply
                        | OpCode::Divide
                        | OpCode::Modulo
                )
            {
                for (var_idx, lit_idx) in [(0, 1), (1, 0)] {
                    if let CompiledNode::CompiledVar {
                        scope_level: 0,
                        segments,
                        reduce_hint: crate::compiled::ReduceHint::None,
                        metadata_hint: crate::compiled::MetadataHint::None,
                        default_value: None,
                    } = &body_args[var_idx]
                        && segments.is_empty()
                        && let CompiledNode::Value { value: lit_val } = &body_args[lit_idx]
                        && let Some(lit_f) = lit_val.as_f64()
                    {
                        let mut results = Vec::with_capacity(arr.len());
                        for item in &arr {
                            if let Some(item_f) = item.as_f64() {
                                let (lhs, rhs) = if var_idx == 0 {
                                    (item_f, lit_f)
                                } else {
                                    (lit_f, item_f)
                                };
                                let r = match opcode {
                                    OpCode::Add => lhs + rhs,
                                    OpCode::Subtract => lhs - rhs,
                                    OpCode::Multiply => lhs * rhs,
                                    OpCode::Divide => lhs / rhs,
                                    OpCode::Modulo => lhs % rhs,
                                    _ => unreachable!(),
                                };
                                // Preserve integer type when possible
                                if r.fract() == 0.0 && r >= i64::MIN as f64 && r <= i64::MAX as f64
                                {
                                    results.push(Value::from(r as i64));
                                } else {
                                    results.push(Value::from(r));
                                }
                            } else {
                                results.push(Value::Null);
                            }
                        }
                        return Ok(Value::Array(results));
                    }
                }
            }

            let len = arr.len();
            let mut results = Vec::with_capacity(len);
            let mut pushed = false;

            for (index, item) in arr.into_iter().enumerate() {
                if !pushed {
                    context.push_with_index(item, 0);
                    pushed = true;
                } else {
                    context.replace_top_data(item, index);
                }
                let result = engine.evaluate_node(logic, context)?;
                results.push(result);
            }
            if len > 0 {
                context.pop();
            }

            Ok(Value::Array(results))
        }
        Value::Object(obj) => {
            let mut results = Vec::with_capacity(obj.len());

            for (index, (key, value)) in obj.iter().enumerate() {
                if index == 0 {
                    context.push_with_key_index(value.clone(), 0, key.clone());
                } else {
                    context.replace_top_key_data(value.clone(), index, key.clone());
                }
                let result = engine.evaluate_node(logic, context)?;
                results.push(result);
            }
            if !obj.is_empty() {
                context.pop();
            }

            Ok(Value::Array(results))
        }
        Value::Null => Ok(Value::Array(vec![])),
        // For primitive values (number, string, bool), treat as single-element collection
        other => {
            // Use push_with_index to avoid HashMap allocation
            context.push_with_index(other, 0);
            let result = engine.evaluate_node(logic, context)?;
            context.pop();

            Ok(Value::Array(vec![result]))
        }
    }
}

/// The `filter` operator - selects elements that match a condition.
///
/// # Syntax
/// ```json
/// {"filter": [collection, condition]}
/// ```
///
/// # Arguments
/// 1. An array or object to filter
/// 2. A condition logic that returns truthy/falsy for each element
///
/// # Context
/// Similar to `map`, each item becomes the context with index/key metadata.
///
/// # Example with Array
/// ```json
/// {
///   "filter": [
///     [{"age": 17}, {"age": 25}, {"age": 30}],
///     {">=": [{"var": "age"}, 18]}
///   ]
/// }
/// ```
/// Returns: `[{"age": 25}, {"age": 30}]`
///
/// # Example with Object
/// ```json
/// {
///   "filter": [
///     {"a": 10, "b": 5, "c": 20},
///     {">": [{"var": ""}, 8]}
///   ]
/// }
/// ```
/// Returns: `{"a": 10, "c": 20}`
#[inline]
pub fn evaluate_filter(
    args: &[CompiledNode],
    context: &mut ContextStack,
    engine: &DataLogic,
) -> Result<Value> {
    if args.len() != 2 {
        return Err(Error::InvalidArguments(INVALID_ARGS.into()));
    }

    let collection = engine.evaluate_node(&args[0], context)?;
    let predicate = &args[1];

    match collection {
        Value::Array(arr) => {
            // Fast path: detect simple comparison predicates to avoid per-item context push.
            // Handles patterns like: {"===": [{"val": "field"}, "literal"]}
            // and {"===": [{"val": "field"}, {"val": [[N], "parent_field"]}]}
            if let CompiledNode::BuiltinOperator {
                opcode,
                args: pred_args,
            } = predicate
                && pred_args.len() == 2
                && matches!(opcode, OpCode::StrictEquals | OpCode::StrictNotEquals)
            {
                let fast = try_extract_filter_field_cmp(&pred_args[0], &pred_args[1])
                    .or_else(|| try_extract_filter_field_cmp(&pred_args[1], &pred_args[0]));

                if let Some((segments, invariant_node)) = fast {
                    // Evaluate the invariant side once before the loop
                    let invariant_owned;
                    let invariant_ref: &Value = match invariant_node {
                        CompiledNode::Value { value, .. } => value,
                        _ => {
                            // Parent scope access: push dummy frame for correct depth
                            context.push(Value::Null);
                            invariant_owned = engine.evaluate_node(invariant_node, context)?;
                            context.pop();
                            &invariant_owned
                        }
                    };

                    let is_eq = matches!(opcode, OpCode::StrictEquals);
                    let results: Vec<Value> = arr
                        .into_iter()
                        .filter(|item| {
                            let matches = super::variable::try_traverse_segments(item, segments)
                                == Some(invariant_ref);
                            if is_eq { matches } else { !matches }
                        })
                        .collect();
                    return Ok(Value::Array(results));
                }
            }

            // Fast path for ordered comparisons on whole items or fields (>=, >, <, <=)
            if let Some(fast_pred) = FastPredicate::try_detect(predicate) {
                let results: Vec<Value> = arr
                    .into_iter()
                    .filter(|item| fast_pred.evaluate(item))
                    .collect();
                return Ok(Value::Array(results));
            }

            let len = arr.len();
            let mut results = Vec::with_capacity(arr.len());
            let mut pushed = false;

            for (index, item) in arr.into_iter().enumerate() {
                if !pushed {
                    context.push_with_index(item, 0);
                    pushed = true;
                } else {
                    context.replace_top_data(item, index);
                }
                let keep = engine.evaluate_node(predicate, context)?;

                if is_truthy(&keep, engine) {
                    // Move data out of context frame instead of cloning
                    results.push(context.take_top_data());
                }
            }
            if len > 0 {
                context.pop();
            }

            Ok(Value::Array(results))
        }
        Value::Object(obj) => {
            let mut result_obj = serde_json::Map::new();

            for (index, (key, value)) in obj.iter().enumerate() {
                if index == 0 {
                    context.push_with_key_index(value.clone(), 0, key.clone());
                } else {
                    context.replace_top_key_data(value.clone(), index, key.clone());
                }
                let keep = engine.evaluate_node(predicate, context)?;

                if is_truthy(&keep, engine) {
                    result_obj.insert(key.clone(), value.clone());
                }
            }
            if !obj.is_empty() {
                context.pop();
            }

            Ok(Value::Object(result_obj))
        }
        Value::Null => Ok(Value::Array(vec![])),
        _ => Err(Error::InvalidArguments(INVALID_ARGS.into())),
    }
}

/// The `reduce` operator - reduces a collection to a single value.
///
/// # Syntax
/// ```json
/// {"reduce": [collection, logic, initial_value]}
/// ```
///
/// # Arguments
/// 1. An array or object to reduce
/// 2. Reduction logic with access to `current` and `accumulator`
/// 3. Initial value for the accumulator
///
/// # Context Variables
/// - `{"var": "current"}` - current element value
/// - `{"var": "accumulator"}` - accumulated value
/// - `{"var": "index"}` - current index or key
///
/// # Example - Sum Array
/// ```json
/// {
///   "reduce": [
///     [1, 2, 3, 4],
///     {"+": [{"var": "accumulator"}, {"var": "current"}]},
///     0
///   ]
/// }
/// ```
/// Returns: `10`
#[inline]
pub fn evaluate_reduce(
    args: &[CompiledNode],
    context: &mut ContextStack,
    engine: &DataLogic,
) -> Result<Value> {
    if args.len() != 3 {
        return Err(Error::InvalidArguments(INVALID_ARGS.into()));
    }

    let array = engine.evaluate_node(&args[0], context)?;
    let logic = &args[1];
    let initial = engine.evaluate_node(&args[2], context)?;

    match array {
        Value::Array(arr) => {
            if arr.is_empty() {
                return Ok(initial);
            }

            // Fast path: detect {op: [val("current"), val("accumulator")]} or reversed
            if let CompiledNode::BuiltinOperator {
                opcode,
                args: body_args,
            } = logic
                && body_args.len() == 2
                && matches!(opcode, OpCode::Add | OpCode::Multiply | OpCode::Subtract)
                && let Some(result) = try_reduce_fast_path(&arr, &initial, body_args, *opcode)
            {
                return Ok(result);
            }

            let len = arr.len();
            let mut accumulator = initial;
            let mut pushed = false;

            for current in arr.into_iter() {
                if !pushed {
                    context.push_reduce(current, accumulator);
                    pushed = true;
                } else {
                    context.replace_reduce_data(current, accumulator);
                }
                accumulator = engine.evaluate_node(logic, context)?;
            }
            if len > 0 {
                context.pop();
            }

            Ok(accumulator)
        }
        Value::Null => Ok(initial),
        _ => Err(Error::InvalidArguments(INVALID_ARGS.into())),
    }
}

/// The `all` operator - checks if all elements satisfy a condition.
///
/// # Syntax
/// ```json
/// {"all": [collection, condition]}
/// ```
///
/// # Arguments
/// 1. An array or object to test
/// 2. A condition to evaluate for each element
///
/// # Returns
/// - `true` if all elements satisfy the condition
/// - `true` if the collection is empty
/// - `false` if any element fails the condition
///
/// # Example
/// ```json
/// {
///   "all": [
///     [10, 20, 30],
///     {">": [{"var": ""}, 5]}
///   ]
/// }
/// ```
/// Returns: `true` (all are greater than 5)
#[inline]
pub fn evaluate_all(
    args: &[CompiledNode],
    context: &mut ContextStack,
    engine: &DataLogic,
) -> Result<Value> {
    if args.len() != 2 {
        return Err(Error::InvalidArguments(INVALID_ARGS.into()));
    }

    let collection = engine.evaluate_node(&args[0], context)?;
    let predicate = &args[1];

    match collection {
        Value::Array(arr) if !arr.is_empty() => {
            // Fast path: detect simple comparison predicates
            if let Some(fast_pred) = FastPredicate::try_detect(predicate) {
                return Ok(Value::Bool(arr.iter().all(|item| fast_pred.evaluate(item))));
            }

            let len = arr.len();
            let mut pushed = false;
            for (index, item) in arr.into_iter().enumerate() {
                if !pushed {
                    context.push_with_index(item, 0);
                    pushed = true;
                } else {
                    context.replace_top_data(item, index);
                }
                let result = engine.evaluate_node(predicate, context)?;

                if !is_truthy(&result, engine) {
                    context.pop();
                    return Ok(Value::Bool(false));
                }
            }
            if len > 0 {
                context.pop();
            }
            Ok(Value::Bool(true))
        }
        Value::Array(arr) if arr.is_empty() => Ok(Value::Bool(false)),
        Value::Null => Ok(Value::Bool(false)),
        _ => Err(Error::InvalidArguments(INVALID_ARGS.into())),
    }
}

/// The `some` operator - checks if any element satisfies a condition.
///
/// # Syntax
/// ```json
/// {"some": [collection, condition]}
/// ```
///
/// # Arguments
/// 1. An array or object to test
/// 2. A condition to evaluate for each element
///
/// # Returns
/// - `true` if any element satisfies the condition
/// - `false` if no elements satisfy or collection is empty
///
/// # Example
/// ```json
/// {
///   "some": [
///     [{"status": "pending"}, {"status": "active"}],
///     {"==": [{"var": "status"}, "active"]}
///   ]
/// }
/// ```
/// Returns: `true`
#[inline]
pub fn evaluate_some(
    args: &[CompiledNode],
    context: &mut ContextStack,
    engine: &DataLogic,
) -> Result<Value> {
    if args.len() != 2 {
        return Err(Error::InvalidArguments(INVALID_ARGS.into()));
    }

    let collection = engine.evaluate_node(&args[0], context)?;
    let predicate = &args[1];

    match collection {
        Value::Array(arr) => {
            // Fast path: detect simple comparison predicates
            if let Some(fast_pred) = FastPredicate::try_detect(predicate) {
                return Ok(Value::Bool(arr.iter().any(|item| fast_pred.evaluate(item))));
            }

            let len = arr.len();
            let mut pushed = false;
            for (index, item) in arr.into_iter().enumerate() {
                if !pushed {
                    context.push_with_index(item, 0);
                    pushed = true;
                } else {
                    context.replace_top_data(item, index);
                }
                let result = engine.evaluate_node(predicate, context)?;

                if is_truthy(&result, engine) {
                    context.pop();
                    return Ok(Value::Bool(true));
                }
            }
            if len > 0 {
                context.pop();
            }
            Ok(Value::Bool(false))
        }
        Value::Null => Ok(Value::Bool(false)),
        _ => Err(Error::InvalidArguments(INVALID_ARGS.into())),
    }
}

/// The `none` operator - checks if no elements satisfy a condition.
///
/// # Syntax
/// ```json
/// {"none": [collection, condition]}
/// ```
///
/// # Arguments
/// 1. An array or object to test
/// 2. A condition to evaluate for each element
///
/// # Returns
/// - `true` if no elements satisfy the condition
/// - `true` if the collection is empty
/// - `false` if any element satisfies the condition
///
/// # Example
/// ```json
/// {
///   "none": [
///     [1, 3, 5, 7],
///     {"==": [{"%": [{"var": ""}, 2]}, 0]}
///   ]
/// }
/// ```
/// Returns: `true` (none are even)
#[inline]
pub fn evaluate_none(
    args: &[CompiledNode],
    context: &mut ContextStack,
    engine: &DataLogic,
) -> Result<Value> {
    if args.len() != 2 {
        return Err(Error::InvalidArguments(INVALID_ARGS.into()));
    }

    let collection = engine.evaluate_node(&args[0], context)?;
    let predicate = &args[1];

    match collection {
        Value::Array(arr) => {
            // Fast path: detect simple comparison predicates
            if let Some(fast_pred) = FastPredicate::try_detect(predicate) {
                return Ok(Value::Bool(
                    !arr.iter().any(|item| fast_pred.evaluate(item)),
                ));
            }

            let len = arr.len();
            let mut pushed = false;
            for (index, item) in arr.into_iter().enumerate() {
                if !pushed {
                    context.push_with_index(item, 0);
                    pushed = true;
                } else {
                    context.replace_top_data(item, index);
                }
                let result = engine.evaluate_node(predicate, context)?;

                if is_truthy(&result, engine) {
                    context.pop();
                    return Ok(Value::Bool(false));
                }
            }
            if len > 0 {
                context.pop();
            }
            Ok(Value::Bool(true))
        }
        Value::Null => Ok(Value::Bool(true)),
        _ => Err(Error::InvalidArguments(INVALID_ARGS.into())),
    }
}

/// The `sort` operator - sorts array elements.
///
/// # Syntax
/// ```json
/// {"sort": [array, accessor]}
/// ```
///
/// # Arguments
/// 1. An array to sort
/// 2. Optional: An accessor to extract sort key from each element
///
/// # Behavior
/// - Without accessor: sorts primitives directly
/// - With accessor: sorts by the extracted value
/// - Sorts in ascending order
/// - Maintains stable sort order
/// - Handles mixed types (nulls first, then bools, numbers, strings, arrays, objects)
///
/// # Example
/// ```json
/// {
///   "sort": [
///     [{"name": "Charlie", "age": 30}, {"name": "Alice", "age": 25}],
///     {"var": "name"}
///   ]
/// }
/// ```
/// Returns: Sorted by name alphabetically
#[inline]
pub fn evaluate_sort(
    args: &[CompiledNode],
    context: &mut ContextStack,
    engine: &DataLogic,
) -> Result<Value> {
    if args.is_empty() {
        return Err(Error::InvalidArguments(INVALID_ARGS.into()));
    }

    // Check if the first argument is a Value node containing null
    if let CompiledNode::Value { value, .. } = &args[0]
        && value.is_null()
    {
        return Err(Error::InvalidArguments(INVALID_ARGS.into()));
    }

    // Evaluate the array
    let array_value = engine.evaluate_node(&args[0], context)?;

    let mut array = match array_value {
        Value::Array(arr) => arr,
        Value::Null => return Ok(Value::Null), // Missing variable returns null
        _ => return Err(Error::InvalidArguments(INVALID_ARGS.into())),
    };

    // Get sort direction (default ascending)
    let ascending = if args.len() > 1 {
        let dir = engine.evaluate_node(&args[1], context)?;
        match dir {
            Value::Bool(b) => b,
            _ => true, // Default to ascending for invalid direction
        }
    } else {
        true
    };

    // Check if we have a field extractor (for sorting objects)
    let has_extractor = args.len() > 2;

    if has_extractor {
        // Sort objects by extracted field
        let extractor = &args[2];

        // Fast path: if extractor is a simple var/val field access,
        // extract keys directly from items without cloning into context.
        // This avoids expensive object cloning (N × ~100ns for complex objects).
        let keys = if let CompiledNode::CompiledVar {
            scope_level: 0,
            segments,
            reduce_hint: crate::compiled::ReduceHint::None,
            metadata_hint: crate::compiled::MetadataHint::None,
            default_value: None,
        } = extractor
        {
            if !segments.is_empty() {
                let mut keys: Vec<Value> = Vec::with_capacity(array.len());
                for item in array.iter() {
                    let key = super::variable::try_traverse_segments(item, segments)
                        .cloned()
                        .unwrap_or(Value::Null);
                    keys.push(key);
                }
                Some(keys)
            } else {
                None
            }
        } else {
            None
        };

        let keys = if let Some(k) = keys {
            k
        } else {
            // General path: push each item into context and evaluate extractor
            let mut keys: Vec<Value> = Vec::with_capacity(array.len());
            let mut pushed = false;

            for (index, item) in array.iter().enumerate() {
                if !pushed {
                    context.push_with_index(item.clone(), 0);
                    pushed = true;
                } else {
                    context.replace_top_data(item.clone(), index);
                }
                keys.push(engine.evaluate_node(extractor, context)?);
            }
            if pushed {
                context.pop();
            }
            keys
        };

        // Build index array and sort by extracted keys
        let mut indices: Vec<usize> = (0..array.len()).collect();
        indices.sort_by(|&a, &b| {
            let cmp = compare_values(&keys[a], &keys[b]);
            if ascending { cmp } else { cmp.reverse() }
        });

        // Reorder array by sorted indices
        let mut sorted = Vec::with_capacity(array.len());
        for i in indices {
            sorted.push(std::mem::replace(&mut array[i], Value::Null));
        }
        array = sorted;
    } else {
        // Sort primitive values directly
        array.sort_by(|a, b| {
            let cmp = compare_values(a, b);
            if ascending { cmp } else { cmp.reverse() }
        });
    }

    Ok(Value::Array(array))
}

/// The `slice` operator - extracts a portion of an array or string.
///
/// # Syntax
/// ```json
/// {"slice": [sequence, start, end]}
/// ```
///
/// # Arguments
/// 1. An array or string to slice
/// 2. Start index (inclusive)
/// 3. Optional: End index (exclusive)
///
/// # Behavior
/// - Negative indices count from the end (-1 is last element)
/// - If end is omitted, slices to the end
/// - Returns empty result if indices are out of bounds
/// - Works with both arrays and strings
///
/// # Example with Array
/// ```json
/// {
///   "slice": [
///     ["a", "b", "c", "d", "e"],
///     1,
///     3
///   ]
/// }
/// ```
/// Returns: `["b", "c"]`
///
/// # Example with String
/// ```json
/// {
///   "slice": [
///     "hello world",
///     0,
///     5
///   ]
/// }
/// ```
/// Returns: `"hello"`
#[inline]
pub fn evaluate_slice(
    args: &[CompiledNode],
    context: &mut ContextStack,
    engine: &DataLogic,
) -> Result<Value> {
    if args.is_empty() {
        return Err(Error::InvalidArguments(INVALID_ARGS.into()));
    }

    // Evaluate the collection
    let collection = engine.evaluate_node(&args[0], context)?;

    // Handle null/missing values
    if collection == Value::Null {
        return Ok(Value::Null);
    }

    // Get start index (default to 0 or end for negative step)
    let start = if args.len() > 1 {
        let start_val = engine.evaluate_node(&args[1], context)?;
        match start_val {
            Value::Number(n) => n.as_i64(),
            Value::Null => None,
            _ => return Err(Error::InvalidArguments("NaN".to_string())),
        }
    } else {
        None
    };

    // Get end index (default to length)
    let end = if args.len() > 2 {
        let end_val = engine.evaluate_node(&args[2], context)?;
        match end_val {
            Value::Number(n) => n.as_i64(),
            Value::Null => None,
            _ => return Err(Error::InvalidArguments("NaN".to_string())),
        }
    } else {
        None
    };

    // Get step (default to 1)
    let step = if args.len() > 3 {
        let step_val = engine.evaluate_node(&args[3], context)?;
        match step_val {
            Value::Number(n) => {
                let s = n.as_i64().unwrap_or(1);
                if s == 0 {
                    return Err(Error::InvalidArguments(INVALID_ARGS.into()));
                }
                s
            }
            _ => 1,
        }
    } else {
        1
    };

    match collection {
        Value::Array(arr) => {
            let len = arr.len() as i64;
            let result = slice_sequence(&arr, len, start, end, step);
            Ok(Value::Array(result))
        }
        Value::String(s) => {
            let chars: Vec<char> = s.chars().collect();
            let len = chars.len() as i64;
            let result_string = slice_chars(&chars, len, start, end, step);
            Ok(Value::String(result_string))
        }
        _ => Err(Error::InvalidArguments(INVALID_ARGS.into())),
    }
}

// Helper function to compare JSON values for sorting
fn compare_values(a: &Value, b: &Value) -> Ordering {
    match (a, b) {
        // Null is less than everything
        (Value::Null, Value::Null) => Ordering::Equal,
        (Value::Null, _) => Ordering::Less,
        (_, Value::Null) => Ordering::Greater,

        // Booleans
        (Value::Bool(a), Value::Bool(b)) => a.cmp(b),

        // Numbers
        (Value::Number(a), Value::Number(b)) => {
            let a_f = a.as_f64().unwrap_or(0.0);
            let b_f = b.as_f64().unwrap_or(0.0);
            if a_f < b_f {
                Ordering::Less
            } else if a_f > b_f {
                Ordering::Greater
            } else {
                Ordering::Equal
            }
        }

        // Strings
        (Value::String(a), Value::String(b)) => a.cmp(b),

        // Mixed types - use type order: null < bool < number < string < array < object
        (Value::Bool(_), Value::Number(_)) => Ordering::Less,
        (Value::Bool(_), Value::String(_)) => Ordering::Less,
        (Value::Bool(_), Value::Array(_)) => Ordering::Less,
        (Value::Bool(_), Value::Object(_)) => Ordering::Less,

        (Value::Number(_), Value::Bool(_)) => Ordering::Greater,
        (Value::Number(_), Value::String(_)) => Ordering::Less,
        (Value::Number(_), Value::Array(_)) => Ordering::Less,
        (Value::Number(_), Value::Object(_)) => Ordering::Less,

        (Value::String(_), Value::Bool(_)) => Ordering::Greater,
        (Value::String(_), Value::Number(_)) => Ordering::Greater,
        (Value::String(_), Value::Array(_)) => Ordering::Less,
        (Value::String(_), Value::Object(_)) => Ordering::Less,

        (Value::Array(_), _) => Ordering::Greater,
        (_, Value::Array(_)) => Ordering::Less,

        // Objects are greater than everything else (except other objects)
        (Value::Object(_), Value::Object(_)) => Ordering::Equal,
        (Value::Object(_), _) => Ordering::Greater,
    }
}

// Helper function to slice a sequence with start, end, and step
fn slice_sequence(
    arr: &[Value],
    len: i64,
    start: Option<i64>,
    end: Option<i64>,
    step: i64,
) -> Vec<Value> {
    let mut result = Vec::new();

    // Normalize indices with overflow protection
    let (actual_start, actual_end) = if step > 0 {
        let s = normalize_index(start.unwrap_or(0), len);
        let e = normalize_index(end.unwrap_or(len), len);
        (s, e)
    } else {
        // For negative step, defaults are reversed
        // Use saturating_sub to prevent underflow
        let default_start = len.saturating_sub(1);
        let s = normalize_index(start.unwrap_or(default_start), len);
        let e = if let Some(e) = end {
            normalize_index(e, len)
        } else {
            -1 // Go all the way to the beginning
        };
        (s, e)
    };

    // Collect elements with overflow-safe iteration
    if step > 0 {
        let mut i = actual_start;
        while i < actual_end && i < len {
            if i >= 0 && (i as usize) < arr.len() {
                result.push(arr[i as usize].clone());
            }
            // Use saturating_add to prevent overflow
            i = i.saturating_add(step);
            // Break if we've wrapped around
            if step > 0 && i < actual_start {
                break;
            }
        }
    } else {
        let mut i = actual_start;
        while i > actual_end && i >= 0 && i < len {
            if (i as usize) < arr.len() {
                result.push(arr[i as usize].clone());
            }
            // Use saturating_add for negative step (step is negative)
            let next_i = i.saturating_add(step);
            // Break if we've wrapped around
            if step < 0 && next_i > i {
                break;
            }
            i = next_i;
        }
    }

    result
}

// Helper function to slice characters directly without Value conversion
fn slice_chars(
    chars: &[char],
    len: i64,
    start: Option<i64>,
    end: Option<i64>,
    step: i64,
) -> String {
    let mut result = String::new();

    let (actual_start, actual_end) = if step > 0 {
        let s = normalize_index(start.unwrap_or(0), len);
        let e = normalize_index(end.unwrap_or(len), len);
        (s, e)
    } else {
        let default_start = len.saturating_sub(1);
        let s = normalize_index(start.unwrap_or(default_start), len);
        let e = if let Some(e) = end {
            normalize_index(e, len)
        } else {
            -1
        };
        (s, e)
    };

    if step > 0 {
        let mut i = actual_start;
        while i < actual_end && i < len {
            if i >= 0 && (i as usize) < chars.len() {
                result.push(chars[i as usize]);
            }
            i = i.saturating_add(step);
            if step > 0 && i < actual_start {
                break;
            }
        }
    } else {
        let mut i = actual_start;
        while i > actual_end && i >= 0 && i < len {
            if (i as usize) < chars.len() {
                result.push(chars[i as usize]);
            }
            let next_i = i.saturating_add(step);
            if step < 0 && next_i > i {
                break;
            }
            i = next_i;
        }
    }

    result
}

// Helper function to normalize slice indices with overflow protection
fn normalize_index(index: i64, len: i64) -> i64 {
    if index < 0 {
        // Use saturating_add to prevent overflow when index is very negative
        let adjusted = len.saturating_add(index);
        adjusted.max(0)
    } else {
        index.min(len)
    }
}

// ============================================================================
// Traced versions of iteration operators for step-by-step debugging
// ============================================================================

/// Traced version of `map` operator that records iteration steps.
#[inline]
pub fn evaluate_map_traced(
    args: &[CompiledNode],
    context: &mut ContextStack,
    engine: &DataLogic,
    collector: &mut TraceCollector,
    node_id_map: &HashMap<usize, u32>,
) -> Result<Value> {
    if args.len() != 2 {
        return Err(Error::InvalidArguments(INVALID_ARGS.into()));
    }

    let collection = engine.evaluate_node_traced(&args[0], context, collector, node_id_map)?;
    let logic = &args[1];

    match &collection {
        Value::Array(arr) => {
            let total = arr.len() as u32;
            let mut results = Vec::with_capacity(arr.len());

            for (index, item) in arr.iter().enumerate() {
                if index == 0 {
                    context.push_with_index(item.clone(), 0);
                } else {
                    context.replace_top_data(item.clone(), index);
                }
                collector.push_iteration(index as u32, total);

                let result = engine.evaluate_node_traced(logic, context, collector, node_id_map)?;
                results.push(result);

                collector.pop_iteration();
            }
            if !arr.is_empty() {
                context.pop();
            }

            Ok(Value::Array(results))
        }
        Value::Object(obj) => {
            let total = obj.len() as u32;
            let mut results = Vec::with_capacity(obj.len());

            for (index, (key, value)) in obj.iter().enumerate() {
                if index == 0 {
                    context.push_with_key_index(value.clone(), 0, key.clone());
                } else {
                    context.replace_top_key_data(value.clone(), index, key.clone());
                }
                collector.push_iteration(index as u32, total);

                let result = engine.evaluate_node_traced(logic, context, collector, node_id_map)?;
                results.push(result);

                collector.pop_iteration();
            }
            if !obj.is_empty() {
                context.pop();
            }

            Ok(Value::Array(results))
        }
        Value::Null => Ok(Value::Array(vec![])),
        _ => {
            context.push_with_index(collection, 0);
            collector.push_iteration(0, 1);

            let result = engine.evaluate_node_traced(logic, context, collector, node_id_map)?;

            collector.pop_iteration();
            context.pop();

            Ok(Value::Array(vec![result]))
        }
    }
}

/// Traced version of `filter` operator that records iteration steps.
#[inline]
pub fn evaluate_filter_traced(
    args: &[CompiledNode],
    context: &mut ContextStack,
    engine: &DataLogic,
    collector: &mut TraceCollector,
    node_id_map: &HashMap<usize, u32>,
) -> Result<Value> {
    if args.len() != 2 {
        return Err(Error::InvalidArguments(INVALID_ARGS.into()));
    }

    let collection = engine.evaluate_node_traced(&args[0], context, collector, node_id_map)?;
    let predicate = &args[1];

    match &collection {
        Value::Array(arr) => {
            let total = arr.len() as u32;
            let mut results = Vec::new();

            for (index, item) in arr.iter().enumerate() {
                if index == 0 {
                    context.push_with_index(item.clone(), 0);
                } else {
                    context.replace_top_data(item.clone(), index);
                }
                collector.push_iteration(index as u32, total);

                let keep =
                    engine.evaluate_node_traced(predicate, context, collector, node_id_map)?;

                collector.pop_iteration();

                if is_truthy(&keep, engine) {
                    results.push(item.clone());
                }
            }
            if !arr.is_empty() {
                context.pop();
            }

            Ok(Value::Array(results))
        }
        Value::Object(obj) => {
            let total = obj.len() as u32;
            let mut result_obj = serde_json::Map::new();

            for (index, (key, value)) in obj.iter().enumerate() {
                if index == 0 {
                    context.push_with_key_index(value.clone(), 0, key.clone());
                } else {
                    context.replace_top_key_data(value.clone(), index, key.clone());
                }
                collector.push_iteration(index as u32, total);

                let keep =
                    engine.evaluate_node_traced(predicate, context, collector, node_id_map)?;

                collector.pop_iteration();

                if is_truthy(&keep, engine) {
                    result_obj.insert(key.clone(), value.clone());
                }
            }
            if !obj.is_empty() {
                context.pop();
            }

            Ok(Value::Object(result_obj))
        }
        Value::Null => Ok(Value::Array(vec![])),
        _ => Err(Error::InvalidArguments(INVALID_ARGS.into())),
    }
}

/// Traced version of `reduce` operator that records iteration steps.
#[inline]
pub fn evaluate_reduce_traced(
    args: &[CompiledNode],
    context: &mut ContextStack,
    engine: &DataLogic,
    collector: &mut TraceCollector,
    node_id_map: &HashMap<usize, u32>,
) -> Result<Value> {
    if args.len() != 3 {
        return Err(Error::InvalidArguments(INVALID_ARGS.into()));
    }

    let array = engine.evaluate_node_traced(&args[0], context, collector, node_id_map)?;
    let logic = &args[1];
    let initial = engine.evaluate_node_traced(&args[2], context, collector, node_id_map)?;

    match &array {
        Value::Array(arr) => {
            if arr.is_empty() {
                return Ok(initial);
            }

            let total = arr.len() as u32;
            let mut accumulator = initial;

            for (index, current) in arr.iter().enumerate() {
                if index == 0 {
                    context.push_reduce(current.clone(), accumulator);
                } else {
                    context.replace_reduce_data(current.clone(), accumulator);
                }
                collector.push_iteration(index as u32, total);

                accumulator =
                    engine.evaluate_node_traced(logic, context, collector, node_id_map)?;

                collector.pop_iteration();
            }
            if !arr.is_empty() {
                context.pop();
            }

            Ok(accumulator)
        }
        Value::Null => Ok(initial),
        _ => Err(Error::InvalidArguments(INVALID_ARGS.into())),
    }
}

/// Traced version of `all` operator that records iteration steps.
#[inline]
pub fn evaluate_all_traced(
    args: &[CompiledNode],
    context: &mut ContextStack,
    engine: &DataLogic,
    collector: &mut TraceCollector,
    node_id_map: &HashMap<usize, u32>,
) -> Result<Value> {
    if args.len() != 2 {
        return Err(Error::InvalidArguments(INVALID_ARGS.into()));
    }

    let collection = engine.evaluate_node_traced(&args[0], context, collector, node_id_map)?;
    let predicate = &args[1];

    match &collection {
        Value::Array(arr) if !arr.is_empty() => {
            let total = arr.len() as u32;

            for (index, item) in arr.iter().enumerate() {
                if index == 0 {
                    context.push_with_index(item.clone(), 0);
                } else {
                    context.replace_top_data(item.clone(), index);
                }
                collector.push_iteration(index as u32, total);

                let result =
                    engine.evaluate_node_traced(predicate, context, collector, node_id_map)?;

                collector.pop_iteration();

                if !is_truthy(&result, engine) {
                    context.pop();
                    return Ok(Value::Bool(false));
                }
            }
            context.pop();
            Ok(Value::Bool(true))
        }
        Value::Array(arr) if arr.is_empty() => Ok(Value::Bool(false)),
        Value::Null => Ok(Value::Bool(false)),
        _ => Err(Error::InvalidArguments(INVALID_ARGS.into())),
    }
}

/// Traced version of `some` operator that records iteration steps.
#[inline]
pub fn evaluate_some_traced(
    args: &[CompiledNode],
    context: &mut ContextStack,
    engine: &DataLogic,
    collector: &mut TraceCollector,
    node_id_map: &HashMap<usize, u32>,
) -> Result<Value> {
    if args.len() != 2 {
        return Err(Error::InvalidArguments(INVALID_ARGS.into()));
    }

    let collection = engine.evaluate_node_traced(&args[0], context, collector, node_id_map)?;
    let predicate = &args[1];

    match &collection {
        Value::Array(arr) => {
            let total = arr.len() as u32;

            for (index, item) in arr.iter().enumerate() {
                if index == 0 {
                    context.push_with_index(item.clone(), 0);
                } else {
                    context.replace_top_data(item.clone(), index);
                }
                collector.push_iteration(index as u32, total);

                let result =
                    engine.evaluate_node_traced(predicate, context, collector, node_id_map)?;

                collector.pop_iteration();

                if is_truthy(&result, engine) {
                    context.pop();
                    return Ok(Value::Bool(true));
                }
            }
            if !arr.is_empty() {
                context.pop();
            }
            Ok(Value::Bool(false))
        }
        Value::Null => Ok(Value::Bool(false)),
        _ => Err(Error::InvalidArguments(INVALID_ARGS.into())),
    }
}

/// Traced version of `none` operator that records iteration steps.
#[inline]
pub fn evaluate_none_traced(
    args: &[CompiledNode],
    context: &mut ContextStack,
    engine: &DataLogic,
    collector: &mut TraceCollector,
    node_id_map: &HashMap<usize, u32>,
) -> Result<Value> {
    if args.len() != 2 {
        return Err(Error::InvalidArguments(INVALID_ARGS.into()));
    }

    let collection = engine.evaluate_node_traced(&args[0], context, collector, node_id_map)?;
    let predicate = &args[1];

    match &collection {
        Value::Array(arr) => {
            let total = arr.len() as u32;

            for (index, item) in arr.iter().enumerate() {
                if index == 0 {
                    context.push_with_index(item.clone(), 0);
                } else {
                    context.replace_top_data(item.clone(), index);
                }
                collector.push_iteration(index as u32, total);

                let result =
                    engine.evaluate_node_traced(predicate, context, collector, node_id_map)?;

                collector.pop_iteration();

                if is_truthy(&result, engine) {
                    context.pop();
                    return Ok(Value::Bool(false));
                }
            }
            if !arr.is_empty() {
                context.pop();
            }
            Ok(Value::Bool(true))
        }
        Value::Null => Ok(Value::Bool(true)),
        _ => Err(Error::InvalidArguments(INVALID_ARGS.into())),
    }
}
