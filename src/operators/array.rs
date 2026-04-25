use serde_json::Value;

#[cfg(feature = "ext-array")]
use std::cmp::Ordering;

use super::helpers::is_truthy;
use super::variable;
use crate::constants::INVALID_ARGS;
use crate::eval_mode::Mode;
use crate::node::{MetadataHint, ReduceHint};
use crate::opcode::OpCode;
use crate::arena::{ArenaContextStack, ArenaValue, value_to_arena};
use crate::{CompiledNode, ContextStack, DataLogic, Error, Result};
use bumpalo::Bump;

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
) -> Option<(&'a [crate::node::PathSegment], &'a CompiledNode)> {
    if let CompiledNode::CompiledVar {
        scope_level: 0,
        segments,
        reduce_hint: ReduceHint::None,
        metadata_hint: MetadataHint::None,
        default_value: None,
        ..
    } = a
        && !segments.is_empty()
        && is_filter_invariant(b)
    {
        return Some((segments, b));
    }
    None
}

/// Evaluate a loop-invariant node without pushing a dummy frame.
/// For CompiledVar with scope_level > 0, uses scope_level - 1 to compensate for
/// the missing dummy frame (get_at_level(N) with D+1 frames == get_at_level(N-1) with D frames).
/// Falls back to dummy push/pop for other node types.
fn evaluate_invariant_no_push(
    invariant_node: &CompiledNode,
    context: &mut ContextStack,
    engine: &DataLogic,
) -> Result<Value> {
    match invariant_node {
        CompiledNode::Value { value, .. } => Ok(value.clone()),
        CompiledNode::CompiledVar {
            scope_level,
            segments,
            reduce_hint: ReduceHint::None,
            metadata_hint: MetadataHint::None,
            default_value,
            ..
        } if *scope_level > 0 => variable::evaluate_compiled_var(
            scope_level - 1,
            segments,
            ReduceHint::None,
            MetadataHint::None,
            default_value.as_deref(),
            context,
            engine,
        ),
        _ => {
            context.push(Value::Null);
            let result = engine.evaluate_node(invariant_node, context)?;
            context.pop();
            Ok(result)
        }
    }
}

/// Represents a detected fast-path predicate pattern for quantifier/filter operators.
/// Avoids per-item context push/pop and evaluate_node dispatch.
/// When `segments` is `Some`, compares a field extracted via path traversal;
/// when `None`, compares the whole item directly.
enum FastPredicate<'a> {
    /// Strict equality (===) or inequality (!==) against a literal
    StrictEq {
        segments: Option<&'a [crate::node::PathSegment]>,
        literal: &'a Value,
        negate: bool,
    },
    /// Ordered numeric comparison (>, >=, <, <=) against a numeric literal
    NumericCmp {
        segments: Option<&'a [crate::node::PathSegment]>,
        literal_f: f64,
        opcode: OpCode,
        var_is_lhs: bool,
    },
    /// Loose numeric equality (==) or inequality (!=) against a numeric literal
    LooseNumericEq {
        segments: Option<&'a [crate::node::PathSegment]>,
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
            ..
        } = predicate
            && pred_args.len() == 2
        {
            // Try both orderings: (var, literal) and (literal, var)
            for (var_idx, lit_idx, var_is_lhs) in [(0, 1, true), (1, 0, false)] {
                if let CompiledNode::CompiledVar {
                    scope_level: 0,
                    segments,
                    reduce_hint: ReduceHint::None,
                    metadata_hint: MetadataHint::None,
                    default_value: None,
                    ..
                } = &pred_args[var_idx]
                    && let CompiledNode::Value { value: literal, .. } = &pred_args[lit_idx]
                {
                    let segs = if segments.is_empty() {
                        None
                    } else {
                        Some(&**segments)
                    };

                    match opcode {
                        OpCode::StrictEquals | OpCode::StrictNotEquals => {
                            let negate = matches!(opcode, OpCode::StrictNotEquals);
                            return Some(FastPredicate::StrictEq {
                                segments: segs,
                                literal,
                                negate,
                            });
                        }
                        OpCode::Equals | OpCode::NotEquals => {
                            // For loose equality with numeric literals, we can use a fast
                            // numeric comparison (loose == is same as strict for numbers)
                            if let Some(lit_f) = literal.as_f64() {
                                let negate = matches!(opcode, OpCode::NotEquals);
                                return Some(FastPredicate::LooseNumericEq {
                                    segments: segs,
                                    literal_f: lit_f,
                                    negate,
                                });
                            }
                        }
                        OpCode::GreaterThan
                        | OpCode::GreaterThanEqual
                        | OpCode::LessThan
                        | OpCode::LessThanEqual => {
                            if let Some(lit_f) = literal.as_f64() {
                                return Some(FastPredicate::NumericCmp {
                                    segments: segs,
                                    literal_f: lit_f,
                                    opcode: *opcode,
                                    var_is_lhs,
                                });
                            }
                        }
                        _ => {}
                    }
                }
            }
        }
        None
    }

    /// Resolve the value to compare: either the whole item or a field within it.
    #[inline]
    fn resolve_value<'v>(
        segments: Option<&[crate::node::PathSegment]>,
        item: &'v Value,
    ) -> Option<&'v Value> {
        match segments {
            None => Some(item),
            Some(segs) => super::variable::try_traverse_segments(item, segs),
        }
    }

    /// Evaluate this predicate against a single item.
    #[inline]
    fn evaluate(&self, item: &Value) -> bool {
        match self {
            FastPredicate::StrictEq {
                segments,
                literal,
                negate,
            } => {
                let matches = Self::resolve_value(*segments, item) == Some(*literal);
                if *negate { !matches } else { matches }
            }
            FastPredicate::NumericCmp {
                segments,
                literal_f,
                opcode,
                var_is_lhs,
            } => {
                if let Some(val) = Self::resolve_value(*segments, item)
                    && let Some(val_f) = val.as_f64()
                {
                    let (lhs, rhs) = if *var_is_lhs {
                        (val_f, *literal_f)
                    } else {
                        (*literal_f, val_f)
                    };
                    inline_numeric_cmp(lhs, rhs, *opcode)
                } else {
                    false
                }
            }
            FastPredicate::LooseNumericEq {
                segments,
                literal_f,
                negate,
            } => {
                let matches = if let Some(val) = Self::resolve_value(*segments, item)
                    && let Some(val_f) = val.as_f64()
                {
                    val_f == *literal_f
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
pub fn evaluate_map<M: Mode>(
    args: &[CompiledNode],
    context: &mut ContextStack,
    engine: &DataLogic,
    mode: &mut M,
) -> Result<Value> {
    if args.len() != 2 {
        return Err(Error::InvalidArguments(INVALID_ARGS.into()));
    }

    let collection = engine.evaluate_node_with_mode::<M>(&args[0], context, mode)?;
    let logic = &args[1];

    match collection {
        Value::Array(arr) => {
            // Fast path: if the map body is a simple var/val access,
            // handle it directly without pushing items into context.
            // Skipped under tracing so per-iteration steps are still recorded.
            if !M::TRACED
                && let CompiledNode::CompiledVar {
                    scope_level: 0,
                    segments,
                    reduce_hint: ReduceHint::None,
                    metadata_hint: MetadataHint::None,
                    default_value: None,
                    ..
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
            if !M::TRACED
                && let CompiledNode::BuiltinOperator {
                    opcode,
                    args: body_args,
                    ..
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
                        reduce_hint: ReduceHint::None,
                        metadata_hint: MetadataHint::None,
                        default_value: None,
                        ..
                    } = &body_args[var_idx]
                        && segments.is_empty()
                        && let CompiledNode::Value { value: lit_val, .. } = &body_args[lit_idx]
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
            let total = len as u32;
            let mut results = Vec::with_capacity(len);
            let mut pushed = false;

            for (index, item) in arr.into_iter().enumerate() {
                if !pushed {
                    context.push_with_index(item, 0);
                    pushed = true;
                } else {
                    context.replace_top_data(item, index);
                }
                mode.push_iteration(index as u32, total);
                let result = engine.evaluate_node_with_mode::<M>(logic, context, mode);
                mode.pop_iteration();
                results.push(result?);
            }
            if len > 0 {
                context.pop();
            }

            Ok(Value::Array(results))
        }
        Value::Object(obj) => {
            let total = obj.len() as u32;
            let len = obj.len();
            let mut results = Vec::with_capacity(len);

            for (index, (key, value)) in obj.into_iter().enumerate() {
                if index == 0 {
                    context.push_with_key_index(value, 0, key);
                } else {
                    context.replace_top_key_data(value, index, key);
                }
                mode.push_iteration(index as u32, total);
                let result = engine.evaluate_node_with_mode::<M>(logic, context, mode);
                mode.pop_iteration();
                results.push(result?);
            }
            if len > 0 {
                context.pop();
            }

            Ok(Value::Array(results))
        }
        Value::Null => Ok(Value::Array(vec![])),
        // For primitive values (number, string, bool), treat as single-element collection
        other => {
            // Use push_with_index to avoid HashMap allocation
            context.push_with_index(other, 0);
            mode.push_iteration(0, 1);
            let result = engine.evaluate_node_with_mode::<M>(logic, context, mode);
            mode.pop_iteration();
            context.pop();

            Ok(Value::Array(vec![result?]))
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
pub fn evaluate_filter<M: Mode>(
    args: &[CompiledNode],
    context: &mut ContextStack,
    engine: &DataLogic,
    mode: &mut M,
) -> Result<Value> {
    if args.len() != 2 {
        return Err(Error::InvalidArguments(INVALID_ARGS.into()));
    }

    let collection = engine.evaluate_node_with_mode::<M>(&args[0], context, mode)?;
    let predicate = &args[1];

    match collection {
        Value::Array(arr) => {
            // Fast path: detect simple comparison predicates to avoid per-item context push.
            // Skipped under tracing so per-iteration steps are still recorded.
            if !M::TRACED
                && let CompiledNode::BuiltinOperator {
                    opcode,
                    args: pred_args,
                    ..
                } = predicate
                && pred_args.len() == 2
                && matches!(opcode, OpCode::StrictEquals | OpCode::StrictNotEquals)
            {
                let fast = try_extract_filter_field_cmp(&pred_args[0], &pred_args[1])
                    .or_else(|| try_extract_filter_field_cmp(&pred_args[1], &pred_args[0]));

                if let Some((segments, invariant_node)) = fast {
                    let invariant_val =
                        evaluate_invariant_no_push(invariant_node, context, engine)?;
                    let is_eq = matches!(opcode, OpCode::StrictEquals);
                    let results: Vec<Value> = arr
                        .into_iter()
                        .filter(|item| {
                            let matches = super::variable::try_traverse_segments(item, segments)
                                == Some(&invariant_val);
                            if is_eq { matches } else { !matches }
                        })
                        .collect();
                    return Ok(Value::Array(results));
                }
            }

            // Fast path for ordered comparisons on whole items or fields (>=, >, <, <=)
            if !M::TRACED
                && let Some(fast_pred) = FastPredicate::try_detect(predicate)
            {
                let results: Vec<Value> = arr
                    .into_iter()
                    .filter(|item| fast_pred.evaluate(item))
                    .collect();
                return Ok(Value::Array(results));
            }

            let len = arr.len();
            let total = len as u32;
            let mut results = Vec::with_capacity(arr.len());
            let mut pushed = false;

            for (index, item) in arr.into_iter().enumerate() {
                if !pushed {
                    context.push_with_index(item, 0);
                    pushed = true;
                } else {
                    context.replace_top_data(item, index);
                }
                mode.push_iteration(index as u32, total);
                let keep = engine.evaluate_node_with_mode::<M>(predicate, context, mode);
                mode.pop_iteration();
                let keep = keep?;

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
            let total = obj.len() as u32;
            let len = obj.len();
            let mut result_obj = serde_json::Map::new();

            for (index, (key, value)) in obj.into_iter().enumerate() {
                if index == 0 {
                    context.push_with_key_index(value.clone(), 0, key.clone());
                } else {
                    context.replace_top_key_data(value.clone(), index, key.clone());
                }
                mode.push_iteration(index as u32, total);
                let keep = engine.evaluate_node_with_mode::<M>(predicate, context, mode);
                mode.pop_iteration();
                let keep = keep?;

                if is_truthy(&keep, engine) {
                    result_obj.insert(key, value);
                }
            }
            if len > 0 {
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
pub fn evaluate_reduce<M: Mode>(
    args: &[CompiledNode],
    context: &mut ContextStack,
    engine: &DataLogic,
    mode: &mut M,
) -> Result<Value> {
    if args.len() != 3 {
        return Err(Error::InvalidArguments(INVALID_ARGS.into()));
    }

    let array = engine.evaluate_node_with_mode::<M>(&args[0], context, mode)?;
    let logic = &args[1];
    let initial = engine.evaluate_node_with_mode::<M>(&args[2], context, mode)?;

    match array {
        Value::Array(arr) => {
            if arr.is_empty() {
                return Ok(initial);
            }

            // Fast path: detect {op: [val("current"), val("accumulator")]} or reversed.
            // Skipped under tracing so per-iteration steps are still recorded.
            if !M::TRACED
                && let CompiledNode::BuiltinOperator {
                    opcode,
                    args: body_args,
                    ..
                } = logic
                && body_args.len() == 2
                && matches!(opcode, OpCode::Add | OpCode::Multiply | OpCode::Subtract)
                && let Some(result) = try_reduce_fast_path(&arr, &initial, body_args, *opcode)
            {
                return Ok(result);
            }

            let len = arr.len();
            let total = len as u32;
            let mut accumulator = initial;
            let mut pushed = false;

            for (index, current) in arr.into_iter().enumerate() {
                if !pushed {
                    context.push_reduce(current, accumulator);
                    pushed = true;
                } else {
                    context.replace_reduce_data(current, accumulator);
                }
                mode.push_iteration(index as u32, total);
                let next = engine.evaluate_node_with_mode::<M>(logic, context, mode);
                mode.pop_iteration();
                accumulator = next?;
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
pub fn evaluate_all<M: Mode>(
    args: &[CompiledNode],
    context: &mut ContextStack,
    engine: &DataLogic,
    mode: &mut M,
) -> Result<Value> {
    if args.len() != 2 {
        return Err(Error::InvalidArguments(INVALID_ARGS.into()));
    }

    let collection = engine.evaluate_node_with_mode::<M>(&args[0], context, mode)?;
    let predicate = &args[1];

    match collection {
        Value::Array(arr) if !arr.is_empty() => {
            // Fast path: field-vs-invariant comparison (avoids per-item context push).
            // Skipped under tracing so per-iteration steps are still recorded.
            if !M::TRACED
                && let CompiledNode::BuiltinOperator {
                    opcode,
                    args: pred_args,
                    ..
                } = predicate
                && pred_args.len() == 2
                && matches!(opcode, OpCode::StrictEquals | OpCode::StrictNotEquals)
            {
                let fast = try_extract_filter_field_cmp(&pred_args[0], &pred_args[1])
                    .or_else(|| try_extract_filter_field_cmp(&pred_args[1], &pred_args[0]));
                if let Some((segments, invariant_node)) = fast {
                    let invariant_val =
                        evaluate_invariant_no_push(invariant_node, context, engine)?;
                    let is_eq = matches!(opcode, OpCode::StrictEquals);
                    return Ok(Value::Bool(arr.iter().all(|item| {
                        let matches = super::variable::try_traverse_segments(item, segments)
                            == Some(&invariant_val);
                        if is_eq { matches } else { !matches }
                    })));
                }
            }

            // Fast path: detect simple comparison predicates
            if !M::TRACED
                && let Some(fast_pred) = FastPredicate::try_detect(predicate)
            {
                return Ok(Value::Bool(arr.iter().all(|item| fast_pred.evaluate(item))));
            }

            let len = arr.len();
            let total = len as u32;
            let mut pushed = false;
            for (index, item) in arr.into_iter().enumerate() {
                if !pushed {
                    context.push_with_index(item, 0);
                    pushed = true;
                } else {
                    context.replace_top_data(item, index);
                }
                mode.push_iteration(index as u32, total);
                let result = engine.evaluate_node_with_mode::<M>(predicate, context, mode);
                mode.pop_iteration();
                let result = result?;

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
pub fn evaluate_some<M: Mode>(
    args: &[CompiledNode],
    context: &mut ContextStack,
    engine: &DataLogic,
    mode: &mut M,
) -> Result<Value> {
    if args.len() != 2 {
        return Err(Error::InvalidArguments(INVALID_ARGS.into()));
    }

    let collection = engine.evaluate_node_with_mode::<M>(&args[0], context, mode)?;
    let predicate = &args[1];

    match collection {
        Value::Array(arr) => {
            // Fast path: field-vs-invariant comparison (avoids per-item context push).
            // Skipped under tracing so per-iteration steps are still recorded.
            if !M::TRACED
                && let CompiledNode::BuiltinOperator {
                    opcode,
                    args: pred_args,
                    ..
                } = predicate
                && pred_args.len() == 2
                && matches!(opcode, OpCode::StrictEquals | OpCode::StrictNotEquals)
            {
                let fast = try_extract_filter_field_cmp(&pred_args[0], &pred_args[1])
                    .or_else(|| try_extract_filter_field_cmp(&pred_args[1], &pred_args[0]));
                if let Some((segments, invariant_node)) = fast {
                    let invariant_val =
                        evaluate_invariant_no_push(invariant_node, context, engine)?;
                    let is_eq = matches!(opcode, OpCode::StrictEquals);
                    return Ok(Value::Bool(arr.iter().any(|item| {
                        let matches = super::variable::try_traverse_segments(item, segments)
                            == Some(&invariant_val);
                        if is_eq { matches } else { !matches }
                    })));
                }
            }

            // Fast path: detect simple comparison predicates
            if !M::TRACED
                && let Some(fast_pred) = FastPredicate::try_detect(predicate)
            {
                return Ok(Value::Bool(arr.iter().any(|item| fast_pred.evaluate(item))));
            }

            let len = arr.len();
            let total = len as u32;
            let mut pushed = false;
            for (index, item) in arr.into_iter().enumerate() {
                if !pushed {
                    context.push_with_index(item, 0);
                    pushed = true;
                } else {
                    context.replace_top_data(item, index);
                }
                mode.push_iteration(index as u32, total);
                let result = engine.evaluate_node_with_mode::<M>(predicate, context, mode);
                mode.pop_iteration();
                let result = result?;

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
pub fn evaluate_none<M: Mode>(
    args: &[CompiledNode],
    context: &mut ContextStack,
    engine: &DataLogic,
    mode: &mut M,
) -> Result<Value> {
    if args.len() != 2 {
        return Err(Error::InvalidArguments(INVALID_ARGS.into()));
    }

    let collection = engine.evaluate_node_with_mode::<M>(&args[0], context, mode)?;
    let predicate = &args[1];

    match collection {
        Value::Array(arr) => {
            // Fast path: field-vs-invariant comparison (avoids per-item context push).
            // Skipped under tracing so per-iteration steps are still recorded.
            if !M::TRACED
                && let CompiledNode::BuiltinOperator {
                    opcode,
                    args: pred_args,
                    ..
                } = predicate
                && pred_args.len() == 2
                && matches!(opcode, OpCode::StrictEquals | OpCode::StrictNotEquals)
            {
                let fast = try_extract_filter_field_cmp(&pred_args[0], &pred_args[1])
                    .or_else(|| try_extract_filter_field_cmp(&pred_args[1], &pred_args[0]));
                if let Some((segments, invariant_node)) = fast {
                    let invariant_val =
                        evaluate_invariant_no_push(invariant_node, context, engine)?;
                    let is_eq = matches!(opcode, OpCode::StrictEquals);
                    return Ok(Value::Bool(!arr.iter().any(|item| {
                        let matches = super::variable::try_traverse_segments(item, segments)
                            == Some(&invariant_val);
                        if is_eq { matches } else { !matches }
                    })));
                }
            }

            // Fast path: detect simple comparison predicates
            if !M::TRACED
                && let Some(fast_pred) = FastPredicate::try_detect(predicate)
            {
                return Ok(Value::Bool(
                    !arr.iter().any(|item| fast_pred.evaluate(item)),
                ));
            }

            let len = arr.len();
            let total = len as u32;
            let mut pushed = false;
            for (index, item) in arr.into_iter().enumerate() {
                if !pushed {
                    context.push_with_index(item, 0);
                    pushed = true;
                } else {
                    context.replace_top_data(item, index);
                }
                mode.push_iteration(index as u32, total);
                let result = engine.evaluate_node_with_mode::<M>(predicate, context, mode);
                mode.pop_iteration();
                let result = result?;

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
#[cfg(feature = "ext-array")]
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
            reduce_hint: ReduceHint::None,
            metadata_hint: MetadataHint::None,
            default_value: None,
            ..
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
            // General path: move each item into context (no clone), evaluate
            // the extractor, then restore the item back into its array slot
            // so the sort-by-indices reorder below still sees the original
            // values.
            let n = array.len();
            let mut keys: Vec<Value> = Vec::with_capacity(n);
            let mut pushed = false;

            for index in 0..n {
                let item = std::mem::replace(&mut array[index], Value::Null);
                if !pushed {
                    context.push_with_index(item, 0);
                    pushed = true;
                } else {
                    let prev = context.take_top_data();
                    array[index - 1] = prev;
                    context.replace_top_data(item, index);
                }
                keys.push(engine.evaluate_node(extractor, context)?);
            }
            if pushed {
                let last = context.take_top_data();
                array[n - 1] = last;
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

#[cfg(feature = "ext-array")]
/// Extract an optional i64 from a CompiledNode, skipping evaluate_node dispatch for literals.
#[inline]
fn extract_opt_i64(
    node: &CompiledNode,
    context: &mut ContextStack,
    engine: &DataLogic,
) -> Result<Option<i64>> {
    match node {
        CompiledNode::Value {
            value: Value::Number(n),
            ..
        } => Ok(n.as_i64()),
        CompiledNode::Value {
            value: Value::Null, ..
        } => Ok(None),
        _ => {
            let val = engine.evaluate_node(node, context)?;
            match val {
                Value::Number(n) => Ok(n.as_i64()),
                Value::Null => Ok(None),
                _ => Err(Error::InvalidArguments("NaN".to_string())),
            }
        }
    }
}

#[cfg(feature = "ext-array")]
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
        extract_opt_i64(&args[1], context, engine)?
    } else {
        None
    };

    // Get end index (default to length)
    let end = if args.len() > 2 {
        extract_opt_i64(&args[2], context, engine)?
    } else {
        None
    };

    // Get step (default to 1)
    let step = if args.len() > 3 {
        let s = extract_opt_i64(&args[3], context, engine)?.unwrap_or(1);
        if s == 0 {
            return Err(Error::InvalidArguments(INVALID_ARGS.into()));
        }
        s
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

#[cfg(feature = "ext-array")]
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

#[cfg(feature = "ext-array")]
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

#[cfg(feature = "ext-array")]
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

/// Native arena-mode `slice`. Returns array slices as `InputRef` slices
/// (zero-copy borrow into the input) when possible; string slices are
/// allocated in the arena.
#[cfg(feature = "ext-array")]
#[inline]
pub(crate) fn evaluate_slice_arena<'a>(
    args: &[CompiledNode],
    actx: &mut ArenaContextStack<'a>,
    context: &mut ContextStack,
    engine: &DataLogic,
    arena: &'a Bump,
) -> Result<&'a crate::arena::ArenaValue<'a>> {
    if args.is_empty() {
        return Err(Error::InvalidArguments(INVALID_ARGS.into()));
    }

    let coll_av = engine.evaluate_arena_node(&args[0], actx, context, arena)?;

    // Null passthrough.
    if matches!(
        coll_av,
        ArenaValue::Null | ArenaValue::InputRef(Value::Null)
    ) {
        return Ok(crate::arena::pool::singleton_null());
    }

    // Resolve start/end/step.
    let start = if args.len() > 1 {
        extract_opt_i64_arena(&args[1], actx, context, engine, arena)?
    } else {
        None
    };
    let end = if args.len() > 2 {
        extract_opt_i64_arena(&args[2], actx, context, engine, arena)?
    } else {
        None
    };
    let step = if args.len() > 3 {
        let s = extract_opt_i64_arena(&args[3], actx, context, engine, arena)?.unwrap_or(1);
        if s == 0 {
            return Err(Error::InvalidArguments(INVALID_ARGS.into()));
        }
        s
    } else {
        1
    };

    // Resolve the input collection. Borrow the source slice when possible
    // so the result can be a view of arena InputRefs.
    let arr_borrow: Option<&'a [Value]> = match coll_av {
        ArenaValue::InputRef(Value::Array(arr)) => Some(arr.as_slice()),
        _ => None,
    };

    if let Some(arr) = arr_borrow {
        let len = arr.len() as i64;
        let indices = slice_indices(len, start, end, step);
        let mut items: bumpalo::collections::Vec<'a, ArenaValue<'a>> =
            bumpalo::collections::Vec::with_capacity_in(indices.len(), arena);
        for i in indices {
            items.push(ArenaValue::InputRef(&arr[i as usize]));
        }
        return Ok(arena.alloc(ArenaValue::Array(items.into_bump_slice())));
    }

    // Composite arena array — slice through the arena items.
    if let ArenaValue::Array(items) = coll_av {
        let len = items.len() as i64;
        let indices = slice_indices(len, start, end, step);
        let mut out: bumpalo::collections::Vec<'a, ArenaValue<'a>> =
            bumpalo::collections::Vec::with_capacity_in(indices.len(), arena);
        for i in indices {
            out.push(crate::arena::value::reborrow_arena_value(&items[i as usize]));
        }
        return Ok(arena.alloc(ArenaValue::Array(out.into_bump_slice())));
    }

    // String slice — allocate result in the arena.
    let s_str: Option<&str> = match coll_av {
        ArenaValue::String(s) => Some(*s),
        ArenaValue::InputRef(Value::String(s)) => Some(s.as_str()),
        _ => None,
    };
    if let Some(s) = s_str {
        let chars: Vec<char> = s.chars().collect();
        let result_string = slice_chars(&chars, chars.len() as i64, start, end, step);
        let s_arena: &'a str = arena.alloc_str(&result_string);
        return Ok(arena.alloc(ArenaValue::String(s_arena)));
    }

    Err(Error::InvalidArguments(INVALID_ARGS.into()))
}

#[cfg(feature = "ext-array")]
#[inline]
fn extract_opt_i64_arena<'a>(
    node: &CompiledNode,
    actx: &mut ArenaContextStack<'a>,
    context: &mut ContextStack,
    engine: &DataLogic,
    arena: &'a Bump,
) -> Result<Option<i64>> {
    if let CompiledNode::Value { value, .. } = node {
        return match value {
            Value::Number(n) => Ok(n.as_i64()),
            Value::Null => Ok(None),
            _ => Err(Error::InvalidArguments("NaN".to_string())),
        };
    }
    let av = engine.evaluate_arena_node(node, actx, context, arena)?;
    match av {
        ArenaValue::Null | ArenaValue::InputRef(Value::Null) => Ok(None),
        _ => match av.as_i64() {
            Some(i) => Ok(Some(i)),
            None => Err(Error::InvalidArguments("NaN".to_string())),
        },
    }
}

/// Index list for a slice given start/end/step. Mirrors the value-mode
/// `slice_sequence` selection logic without materializing values.
#[cfg(feature = "ext-array")]
#[inline]
fn slice_indices(len: i64, start: Option<i64>, end: Option<i64>, step: i64) -> Vec<i64> {
    let mut out = Vec::new();
    let (actual_start, actual_end) = if step > 0 {
        (
            normalize_index(start.unwrap_or(0), len),
            normalize_index(end.unwrap_or(len), len),
        )
    } else {
        let default_start = len.saturating_sub(1);
        let s = normalize_index(start.unwrap_or(default_start), len);
        let e = if let Some(e) = end {
            if e < -len { -1 } else { normalize_index(e, len) }
        } else {
            -1
        };
        (s, e)
    };

    let mut i = actual_start;
    while (step > 0 && i < actual_end) || (step < 0 && i > actual_end) {
        if i >= 0 && i < len {
            out.push(i);
        }
        i += step;
    }
    out
}

#[cfg(feature = "ext-array")]
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

// =============================================================================
// Arena-mode operators (Phase 4: filter / map / quantifiers + composition IN).
//
// These return `&'a ArenaValue<'a>` and may borrow into the caller's input
// `Value` tree via `ArenaValue::InputRef`. Predicate evaluation still uses the
// existing value-mode path (`engine.evaluate_node`) — the arena win comes from
// (a) borrowing the input collection instead of cloning it and (b) staying in
// the arena when our output is consumed by another arena-mode operator.
//
// Phase 4 adds: iterator inputs can themselves be arena-mode op outputs
// (`map(filter(...))`, `length(map(filter))`). The `IterSrc` helper unifies
// `&[Value]` (borrowed input data) and `&[&'a Value]` (extracted from an
// upstream arena op's `InputRef` items) into one iteration interface, so
// each operator's iteration body stays single-version.
// =============================================================================

/// Unified view over an iterator op's input collection. Either points at the
/// caller's input data (`&[Value]`) or at an arena slice of `&Value`s
/// extracted from an upstream arena op's `InputRef` items.
pub(crate) enum IterSrc<'a> {
    /// Direct slice from caller's input data (zero allocs).
    Direct(&'a [Value]),
    /// Arena-allocated slice of references gathered from `ArenaValue::InputRef`
    /// items produced by an upstream arena op.
    Refs(&'a [&'a Value]),
}

impl<'a> IterSrc<'a> {
    #[inline]
    pub(crate) fn len(&self) -> usize {
        match self {
            Self::Direct(s) => s.len(),
            Self::Refs(s) => s.len(),
        }
    }

    #[inline]
    pub(crate) fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// Get item by index. The returned `&'a Value` lives for the arena
    /// lifetime (either through the caller's `Arc<Value>` or via the upstream
    /// op's `InputRef`).
    #[inline]
    pub(crate) fn get(&self, i: usize) -> &'a Value {
        match self {
            Self::Direct(s) => &s[i],
            Self::Refs(s) => s[i],
        }
    }
}

/// Outcome of resolving an iterator op's first arg in arena mode.
pub(crate) enum ResolvedInput<'a> {
    /// Iterable input — proceed with array iteration.
    Iterable(IterSrc<'a>),
    /// Empty/null input — caller returns its empty-collection result.
    Empty,
    /// Object or other non-array input. Carries the resolved arena value
    /// so callers can dispatch natively (object-iteration / error / etc.)
    /// without re-evaluating the arg.
    Bridge(&'a ArenaValue<'a>),
}

/// Resolve `args[0]` for an arena-mode iterator op. Tries (in order):
///   1. Borrow from root data (cheapest — no eval, no alloc)
///   2. Dispatch to arena (when arg is e.g. another filter — composition path)
///   3. Bridge: caller falls back to value-mode for the whole op
///
/// The returned `IterSrc` borrows from the arena (`'a`) and is safe to iterate
/// while the caller mutates `context` for predicate evaluation, because the
/// underlying data lives in either the input `Arc` (held for the call's
/// duration) or arena slices (allocated on the same arena).
pub(crate) fn resolve_iter_input<'a>(
    arg: &CompiledNode,
    actx: &mut ArenaContextStack<'a>,
    context: &mut ContextStack,
    engine: &DataLogic,
    arena: &'a Bump,
) -> Result<ResolvedInput<'a>> {
    // Path 1: root borrow.
    if let Some(borrowed) = try_borrow_collection_from_root(arg, context, actx.root_input()) {
        return Ok(match borrowed {
            Value::Array(arr) => ResolvedInput::Iterable(IterSrc::Direct(arr.as_slice())),
            Value::Null => ResolvedInput::Empty,
            other => ResolvedInput::Bridge(arena.alloc(ArenaValue::InputRef(other))),
        });
    }

    // Path 2: composition — arg is another arena-aware op. Dispatch and inspect.
    if let CompiledNode::BuiltinOperator { opcode, .. } = arg
        && matches!(
            opcode,
            OpCode::Filter
                | OpCode::Map
                | OpCode::All
                | OpCode::Some
                | OpCode::None
                | OpCode::Reduce
                | OpCode::Merge
        )
    {
        let av = engine.evaluate_arena_node(arg, actx, context, arena)?;
        return Ok(arena_value_as_iter(av, arena));
    }

    // Path 3: anything else — evaluate through arena dispatch so the caller
    // can handle the result natively (Object iteration / single-element wrap /
    // error per op semantics).
    let av = engine.evaluate_arena_node(arg, actx, context, arena)?;
    Ok(arena_value_as_iter(av, arena))
}

/// Convert a resolved arena value into an `IterSrc` view, or signal Empty/Bridge.
fn arena_value_as_iter<'a>(av: &'a ArenaValue<'a>, arena: &'a Bump) -> ResolvedInput<'a> {
    match av {
        ArenaValue::InputRef(Value::Array(arr)) => {
            ResolvedInput::Iterable(IterSrc::Direct(arr.as_slice()))
        }
        ArenaValue::InputRef(Value::Null) | ArenaValue::Null => ResolvedInput::Empty,
        ArenaValue::Array(items) => {
            // Items from an arena op are typically `InputRef(&Value)`. Extract
            // them into an arena-allocated `&[&Value]`. If any item is not an
            // InputRef (e.g. a computed Number/Bool from a future arena op),
            // bridge with the original value so the caller can handle it.
            let mut refs: bumpalo::collections::Vec<'a, &'a Value> =
                bumpalo::collections::Vec::with_capacity_in(items.len(), arena);
            for item in items.iter() {
                match item {
                    ArenaValue::InputRef(v) => refs.push(*v),
                    _ => return ResolvedInput::Bridge(av),
                }
            }
            ResolvedInput::Iterable(IterSrc::Refs(refs.into_bump_slice()))
        }
        _ => ResolvedInput::Bridge(av),
    }
}

/// Arena-mode `filter`. POC: handles only the case where the input collection
/// resolves at root scope (the dominant pattern in real workloads). Falls back
/// to the value-mode filter via the dispatch hub for everything else.
#[inline]
pub(crate) fn evaluate_filter_arena<'a>(
    args: &[CompiledNode],
    actx: &mut ArenaContextStack<'a>,
    context: &mut ContextStack,
    engine: &DataLogic,
    arena: &'a Bump,
) -> Result<&'a ArenaValue<'a>> {
    if args.len() != 2 {
        return Err(Error::InvalidArguments(INVALID_ARGS.into()));
    }

    // Resolve input collection. Try the borrow-from-root fast path first;
    // Phase 4: resolve input via unified helper (root borrow OR upstream arena op).
    let src = match resolve_iter_input(&args[0], actx, context, engine, arena)? {
        ResolvedInput::Iterable(s) => s,
        ResolvedInput::Empty => return Ok(arena.alloc(ArenaValue::Array(&[]))),
        ResolvedInput::Bridge(av) => {
            return filter_arena_bridge(av, &args[1], actx, context, engine, arena);
        }
    };

    let predicate = &args[1];
    let len = src.len();
    if len == 0 {
        return Ok(arena.alloc(ArenaValue::Array(&[])));
    }

    // FAST PATH: predicates evaluable by direct field traversal — no context
    // push, no item clone. The arena win materializes here: zero per-item
    // allocations, only the result slice in the arena.
    if let CompiledNode::BuiltinOperator {
        opcode,
        args: pred_args,
        ..
    } = predicate
        && pred_args.len() == 2
        && matches!(opcode, OpCode::StrictEquals | OpCode::StrictNotEquals)
    {
        let fast = try_extract_filter_field_cmp(&pred_args[0], &pred_args[1])
            .or_else(|| try_extract_filter_field_cmp(&pred_args[1], &pred_args[0]));
        if let Some((segments, invariant_node)) = fast {
            let invariant_val = evaluate_invariant_no_push(invariant_node, context, engine)?;
            let is_eq = matches!(opcode, OpCode::StrictEquals);
            let mut results: bumpalo::collections::Vec<'a, ArenaValue<'a>> =
                bumpalo::collections::Vec::with_capacity_in(len, arena);
            for i in 0..len {
                let item = src.get(i);
                let matches =
                    super::variable::try_traverse_segments(item, segments) == Some(&invariant_val);
                if matches == is_eq {
                    results.push(ArenaValue::InputRef(item));
                }
            }
            return Ok(arena.alloc(ArenaValue::Array(results.into_bump_slice())));
        }
    }

    if let Some(fast_pred) = FastPredicate::try_detect(predicate) {
        let mut results: bumpalo::collections::Vec<'a, ArenaValue<'a>> =
            bumpalo::collections::Vec::with_capacity_in(len, arena);
        for i in 0..len {
            let item = src.get(i);
            if fast_pred.evaluate(item) {
                results.push(ArenaValue::InputRef(item));
            }
        }
        return Ok(arena.alloc(ArenaValue::Array(results.into_bump_slice())));
    }

    // GENERAL PATH: zero-clone via ArenaContextStack. Frame data is
    // `&'a ArenaValue<'a>` pointing at `InputRef(item)`; predicate body
    // dispatches through arena and the var-arena reads the frame directly.
    let mut results: bumpalo::collections::Vec<'a, ArenaValue<'a>> =
        bumpalo::collections::Vec::with_capacity_in(len, arena);
    let mut pushed = false;
    for i in 0..len {
        let item = src.get(i);
        let item_av: &'a ArenaValue<'a> = arena.alloc(ArenaValue::InputRef(item));
        if !pushed {
            actx.push_with_index(item_av, 0);
            pushed = true;
        } else {
            actx.replace_top_data(item_av, i);
        }
        let keep = engine.evaluate_arena_node(predicate, actx, context, arena)?;
        if crate::arena::is_truthy_arena(keep, engine) {
            results.push(ArenaValue::InputRef(item));
        }
    }
    if pushed {
        actx.pop();
    }
    Ok(arena.alloc(ArenaValue::Array(results.into_bump_slice())))
}

/// Filter Bridge case — input is an Object or non-array. Object inputs
/// iterate `(key, value)` pairs and accumulate matched pairs into a
/// new arena `Object`; non-array non-object non-null inputs error per
/// value-mode semantics.
#[inline]
fn filter_arena_bridge<'a>(
    input: &'a ArenaValue<'a>,
    predicate: &CompiledNode,
    actx: &mut ArenaContextStack<'a>,
    context: &mut ContextStack,
    engine: &DataLogic,
    arena: &'a Bump,
) -> Result<&'a ArenaValue<'a>> {
    if let ArenaValue::InputRef(Value::Object(obj)) = input {
        let mut kept: bumpalo::collections::Vec<'a, (&'a str, ArenaValue<'a>)> =
            bumpalo::collections::Vec::with_capacity_in(obj.len(), arena);
        let mut pushed = false;
        for (i, (k, v)) in obj.iter().enumerate() {
            let item_av: &'a ArenaValue<'a> = arena.alloc(ArenaValue::InputRef(v));
            let key_arena: &'a str = arena.alloc_str(k);
            if !pushed {
                actx.push_with_key_index(item_av, 0, key_arena);
                pushed = true;
            } else {
                actx.replace_top_key_data(item_av, i, key_arena);
            }
            let keep = engine.evaluate_arena_node(predicate, actx, context, arena)?;
            if crate::arena::is_truthy_arena(keep, engine) {
                kept.push((key_arena, ArenaValue::InputRef(v)));
            }
        }
        if pushed {
            actx.pop();
        }
        return Ok(arena.alloc(ArenaValue::Object(kept.into_bump_slice())));
    }
    Err(Error::InvalidArguments(INVALID_ARGS.into()))
}

/// Map Bridge case — Object inputs iterate (key, value) pairs and
/// produce an arena `Array` of mapped results. Non-array non-object
/// non-null primitives are treated as a single-element collection.
#[inline]
fn map_arena_bridge<'a>(
    input: &'a ArenaValue<'a>,
    body: &CompiledNode,
    actx: &mut ArenaContextStack<'a>,
    context: &mut ContextStack,
    engine: &DataLogic,
    arena: &'a Bump,
) -> Result<&'a ArenaValue<'a>> {
    if let ArenaValue::InputRef(Value::Object(obj)) = input {
        let mut results: bumpalo::collections::Vec<'a, ArenaValue<'a>> =
            bumpalo::collections::Vec::with_capacity_in(obj.len(), arena);
        let mut pushed = false;
        for (i, (k, v)) in obj.iter().enumerate() {
            let item_av: &'a ArenaValue<'a> = arena.alloc(ArenaValue::InputRef(v));
            let key_arena: &'a str = arena.alloc_str(k);
            if !pushed {
                actx.push_with_key_index(item_av, 0, key_arena);
                pushed = true;
            } else {
                actx.replace_top_key_data(item_av, i, key_arena);
            }
            let av = engine.evaluate_arena_node(body, actx, context, arena)?;
            results.push(crate::arena::value::reborrow_arena_value(av));
        }
        if pushed {
            actx.pop();
        }
        return Ok(arena.alloc(ArenaValue::Array(results.into_bump_slice())));
    }
    // Single-element collection (number, string, bool primitive input).
    let item_av: &'a ArenaValue<'a> = input;
    actx.push_with_index(item_av, 0);
    let av = engine.evaluate_arena_node(body, actx, context, arena)?;
    let owned = crate::arena::value::reborrow_arena_value(av);
    actx.pop();
    let slice = arena.alloc_slice_fill_iter(std::iter::once(owned));
    Ok(arena.alloc(ArenaValue::Array(slice)))
}

/// Reduce Bridge case — Object inputs iterate (key, value) pairs.
/// Non-array non-object non-null inputs return the initial value.
#[inline]
fn reduce_arena_bridge<'a>(
    input: &'a ArenaValue<'a>,
    body: &CompiledNode,
    initial: &Value,
    actx: &mut ArenaContextStack<'a>,
    context: &mut ContextStack,
    engine: &DataLogic,
    arena: &'a Bump,
) -> Result<&'a ArenaValue<'a>> {
    if let ArenaValue::InputRef(Value::Object(obj)) = input {
        let mut acc_av: &'a ArenaValue<'a> =
            arena.alloc(crate::arena::value_to_arena(initial, arena));
        let mut pushed = false;
        for (i, (k, v)) in obj.iter().enumerate() {
            let item_av: &'a ArenaValue<'a> = arena.alloc(ArenaValue::InputRef(v));
            let _key = arena.alloc_str(k); // value-mode reduce frame stores
            // current/accumulator only — key isn't exposed here.
            if !pushed {
                actx.push_reduce(item_av, acc_av);
                pushed = true;
            } else {
                actx.replace_reduce_data(item_av, acc_av);
            }
            let _ = i;
            acc_av = engine.evaluate_arena_node(body, actx, context, arena)?;
        }
        if pushed {
            actx.pop();
        }
        return Ok(acc_av);
    }
    // Anything else — return initial.
    Ok(arena.alloc(crate::arena::value_to_arena(initial, arena)))
}

/// Quantifier Bridge case — Object inputs iterate (key, value) pairs.
#[inline]
fn quantifier_arena_bridge<'a>(
    input: &'a ArenaValue<'a>,
    predicate: &CompiledNode,
    short_circuit_on: bool,
    invert_final: bool,
    empty_result: bool,
    actx: &mut ArenaContextStack<'a>,
    context: &mut ContextStack,
    engine: &DataLogic,
    arena: &'a Bump,
) -> Result<&'a ArenaValue<'a>> {
    if let ArenaValue::InputRef(Value::Object(obj)) = input {
        if obj.is_empty() {
            return Ok(arena.alloc(ArenaValue::Bool(empty_result)));
        }
        let mut pushed = false;
        let mut found_short = false;
        for (i, (k, v)) in obj.iter().enumerate() {
            let item_av: &'a ArenaValue<'a> = arena.alloc(ArenaValue::InputRef(v));
            let key_arena: &'a str = arena.alloc_str(k);
            if !pushed {
                actx.push_with_key_index(item_av, 0, key_arena);
                pushed = true;
            } else {
                actx.replace_top_key_data(item_av, i, key_arena);
            }
            let av = engine.evaluate_arena_node(predicate, actx, context, arena)?;
            if crate::arena::is_truthy_arena(av, engine) == short_circuit_on {
                found_short = true;
                break;
            }
        }
        if pushed {
            actx.pop();
        }
        let result = if found_short {
            if invert_final { !short_circuit_on } else { short_circuit_on }
        } else if invert_final {
            short_circuit_on
        } else {
            !short_circuit_on
        };
        return Ok(arena.alloc(ArenaValue::Bool(result)));
    }
    // Anything else — value-mode treats as empty (returns empty_result).
    Ok(arena.alloc(ArenaValue::Bool(empty_result)))
}

/// Arena-mode `sort`. Borrows input via `IterSrc` (no input clone), runs
/// `slice::sort_by` over indices, and emits `ArenaValue::Array` of `InputRef`s
/// pointing at the original items in their sorted order. The win vs the
/// value-mode `evaluate_sort` is the eliminated initial deep-clone of the
/// whole input array (line 1217 in the value-mode path) — for sort that's
/// the dominant cost on object arrays.
///
/// Fast path (extractor is a root-scope `var`): keys come from
/// `try_traverse_segments` returning `&Value` directly, no key clones.
/// General-extractor path bridges to value-mode `evaluate_sort`.
#[cfg(feature = "ext-array")]
#[inline]
pub(crate) fn evaluate_sort_arena<'a>(
    args: &[CompiledNode],
    actx: &mut ArenaContextStack<'a>,
    context: &mut ContextStack,
    engine: &DataLogic,
    arena: &'a Bump,
) -> Result<&'a ArenaValue<'a>> {
    if args.is_empty() {
        return Err(Error::InvalidArguments(INVALID_ARGS.into()));
    }

    // Match the existing value-mode behavior: literal-null first arg is an error.
    if let CompiledNode::Value { value, .. } = &args[0]
        && value.is_null()
    {
        return Err(Error::InvalidArguments(INVALID_ARGS.into()));
    }

    let src = match resolve_iter_input(&args[0], actx, context, engine, arena)? {
        ResolvedInput::Iterable(s) => s,
        ResolvedInput::Empty => return Ok(arena.alloc(ArenaValue::Null)),
        ResolvedInput::Bridge(av) => {
            return sort_arena_from_value(av, args, actx, context, engine, arena);
        }
    };

    let len = src.len();
    if len == 0 {
        return Ok(arena.alloc(ArenaValue::Array(&[])));
    }

    // Sort direction: defaults to ascending; non-Bool means ascending too
    // (matches the value-mode default at line 1230).
    let ascending = if args.len() > 1 {
        let dir = engine.evaluate_arena_node(&args[1], actx, context, arena)?;
        match dir {
            ArenaValue::Bool(b) | ArenaValue::InputRef(Value::Bool(b)) => *b,
            _ => true,
        }
    } else {
        true
    };

    let has_extractor = args.len() > 2;

    if !has_extractor {
        // No extractor — sort items directly by Value order.
        let mut indices: Vec<usize> = (0..len).collect();
        indices.sort_by(|&a, &b| {
            let cmp = compare_values(src.get(a), src.get(b));
            if ascending { cmp } else { cmp.reverse() }
        });
        let slice = arena.alloc_slice_fill_iter(
            indices.into_iter().map(|i| ArenaValue::InputRef(src.get(i))),
        );
        return Ok(arena.alloc(ArenaValue::Array(slice)));
    }

    // Extractor present — fast path for root-scope `var` segments.
    let extractor = &args[2];
    if let CompiledNode::CompiledVar {
        scope_level: 0,
        segments,
        reduce_hint: ReduceHint::None,
        metadata_hint: MetadataHint::None,
        default_value: None,
        ..
    } = extractor
        && !segments.is_empty()
    {
        let mut keyed: Vec<(usize, Option<&Value>)> = (0..len)
            .map(|i| (i, super::variable::try_traverse_segments(src.get(i), segments)))
            .collect();
        keyed.sort_by(|(_, ka), (_, kb)| {
            let cmp = match (ka, kb) {
                (Some(a), Some(b)) => compare_values(a, b),
                (Some(_), None) => Ordering::Greater,
                (None, Some(_)) => Ordering::Less,
                (None, None) => Ordering::Equal,
            };
            if ascending { cmp } else { cmp.reverse() }
        });
        let slice = arena.alloc_slice_fill_iter(
            keyed.into_iter().map(|(i, _)| ArenaValue::InputRef(src.get(i))),
        );
        return Ok(arena.alloc(ArenaValue::Array(slice)));
    }

    // General extractor — push each item into the arena context, evaluate
    // the extractor, collect keys, then sort indices by key. Result emits
    // `InputRef` views into the original input data.
    let mut keys: Vec<Value> = Vec::with_capacity(len);
    let mut pushed = false;
    for i in 0..len {
        let item = src.get(i);
        let item_av: &'a ArenaValue<'a> = arena.alloc(ArenaValue::InputRef(item));
        if !pushed {
            actx.push_with_index(item_av, 0);
            pushed = true;
        } else {
            actx.replace_top_data(item_av, i);
        }
        let key_av = engine.evaluate_arena_node(extractor, actx, context, arena)?;
        keys.push(crate::arena::arena_to_value(key_av));
    }
    if pushed {
        actx.pop();
    }

    let mut indices: Vec<usize> = (0..len).collect();
    indices.sort_by(|&a, &b| {
        let cmp = compare_values(&keys[a], &keys[b]);
        if ascending { cmp } else { cmp.reverse() }
    });
    let slice = arena.alloc_slice_fill_iter(
        indices.into_iter().map(|i| ArenaValue::InputRef(src.get(i))),
    );
    Ok(arena.alloc(ArenaValue::Array(slice)))
}

/// Sort a resolved arena value when the input wasn't borrowable as a
/// flat `&[Value]` — falls into one of: Null (→ Null), Array (→ sort),
/// anything else (→ error). Re-uses the same direction/extractor logic
/// as the borrowed path.
#[cfg(feature = "ext-array")]
#[inline]
fn sort_arena_from_value<'a>(
    av: &'a ArenaValue<'a>,
    args: &[CompiledNode],
    actx: &mut ArenaContextStack<'a>,
    context: &mut ContextStack,
    engine: &DataLogic,
    arena: &'a Bump,
) -> Result<&'a ArenaValue<'a>> {
    // Convert to a flat `Vec<Value>` we can sort. Borrowed cases go
    // straight through; arena cases materialize once.
    let owned: Vec<Value> = match av {
        ArenaValue::Null | ArenaValue::InputRef(Value::Null) => {
            return Ok(crate::arena::pool::singleton_null());
        }
        ArenaValue::InputRef(Value::Array(arr)) => arr.iter().cloned().collect(),
        ArenaValue::Array(items) => items.iter().map(crate::arena::arena_to_value).collect(),
        _ => return Err(Error::InvalidArguments(INVALID_ARGS.into())),
    };
    if owned.is_empty() {
        return Ok(arena.alloc(ArenaValue::Array(&[])));
    }

    let ascending = if args.len() > 1 {
        let dir = engine.evaluate_arena_node(&args[1], actx, context, arena)?;
        match dir {
            ArenaValue::Bool(b) | ArenaValue::InputRef(Value::Bool(b)) => *b,
            _ => true,
        }
    } else {
        true
    };

    if args.len() <= 2 {
        let mut sorted = owned;
        sorted.sort_by(|a, b| {
            let cmp = compare_values(a, b);
            if ascending { cmp } else { cmp.reverse() }
        });
        // Materialize into the arena.
        let items = arena.alloc_slice_fill_iter(
            sorted.into_iter().map(|v| crate::arena::value_to_arena(&v, arena)),
        );
        return Ok(arena.alloc(ArenaValue::Array(items)));
    }

    // Extractor present — push items into arena context, evaluate,
    // collect keys, sort indices.
    let extractor = &args[2];
    let n = owned.len();
    // Promote each item into the arena once so push_with_index can take a
    // reference whose lifetime matches `'a`.
    let arena_items: bumpalo::collections::Vec<'a, ArenaValue<'a>> = bumpalo::collections::Vec::from_iter_in(
        owned.iter().map(|v| crate::arena::value_to_arena(v, arena)),
        arena,
    );
    let arena_items_slice: &'a [ArenaValue<'a>] = arena_items.into_bump_slice();

    let mut keys: Vec<Value> = Vec::with_capacity(n);
    let mut pushed = false;
    for (i, item_av) in arena_items_slice.iter().enumerate() {
        if !pushed {
            actx.push_with_index(item_av, 0);
            pushed = true;
        } else {
            actx.replace_top_data(item_av, i);
        }
        let key_av = engine.evaluate_arena_node(extractor, actx, context, arena)?;
        keys.push(crate::arena::arena_to_value(key_av));
    }
    if pushed {
        actx.pop();
    }

    let mut indices: Vec<usize> = (0..n).collect();
    indices.sort_by(|&a, &b| {
        let cmp = compare_values(&keys[a], &keys[b]);
        if ascending { cmp } else { cmp.reverse() }
    });

    let out = arena.alloc_slice_fill_iter(
        indices
            .into_iter()
            .map(|i| crate::arena::value::reborrow_arena_value(&arena_items_slice[i])),
    );
    Ok(arena.alloc(ArenaValue::Array(out)))
}

/// Arena-mode `length`. Critical for the COMPOSITION test: when called as
/// `length(filter(...))`, the filter result lives in the arena and length
/// just reads the slice length — zero conversion cost on the intermediate.
#[cfg(feature = "ext-string")]
#[inline]
pub(crate) fn evaluate_length_arena<'a>(
    args: &[CompiledNode],
    actx: &mut ArenaContextStack<'a>,
    context: &mut ContextStack,
    engine: &DataLogic,
    arena: &'a Bump,
) -> Result<&'a ArenaValue<'a>> {
    if args.len() != 1 {
        return Err(Error::InvalidArguments(INVALID_ARGS.into()));
    }

    // Recurse into arena dispatcher so composed cases (e.g. length(filter(...)))
    // stay arena-resident on the intermediate.
    let arg = engine.evaluate_arena_node(&args[0], actx, context, arena)?;

    let n: i64 = match arg {
        ArenaValue::String(s) => s.chars().count() as i64,
        ArenaValue::Array(items) => items.len() as i64,
        ArenaValue::InputRef(v) => match v {
            Value::String(s) => s.chars().count() as i64,
            Value::Array(arr) => arr.len() as i64,
            _ => return Err(Error::InvalidArguments(INVALID_ARGS.into())),
        },
        _ => return Err(Error::InvalidArguments(INVALID_ARGS.into())),
    };

    Ok(arena.alloc(ArenaValue::Number(crate::value::NumberValue::from_i64(n))))
}

/// Try to obtain the input collection by borrowing from the caller's root data.
/// Returns `Some(&Value)` when args[0] is a simple root-scope `var` that
/// resolves into the input data. The returned reference lives for the arena
/// lifetime `'a` because `root` is held alive for the call's duration.
#[inline]
fn try_borrow_collection_from_root<'a>(
    arg: &CompiledNode,
    context: &ContextStack,
    root: &'a Value,
) -> Option<&'a Value> {
    if context.depth() != 0 {
        return None; // POC: only root-scope borrows
    }
    if let CompiledNode::CompiledVar {
        scope_level: 0,
        segments,
        reduce_hint: ReduceHint::None,
        metadata_hint: MetadataHint::None,
        default_value: None,
        ..
    } = arg
    {
        if segments.is_empty() {
            return Some(root);
        }
        return variable::try_traverse_segments(root, segments);
    }
    None
}

/// Arena-mode `map`. POC scope: borrow input from root scope. Body fast path
/// for var/field-extract emits InputRef per item with zero iteration allocs.
/// Other body shapes fall through to value-mode evaluate then promote to arena.
#[inline]
pub(crate) fn evaluate_map_arena<'a>(
    args: &[CompiledNode],
    actx: &mut ArenaContextStack<'a>,
    context: &mut ContextStack,
    engine: &DataLogic,
    arena: &'a Bump,
) -> Result<&'a ArenaValue<'a>> {
    if args.len() != 2 {
        return Err(Error::InvalidArguments(INVALID_ARGS.into()));
    }

    let body = &args[1];
    let src = match resolve_iter_input(&args[0], actx, context, engine, arena)? {
        ResolvedInput::Iterable(s) => s,
        ResolvedInput::Empty => return Ok(arena.alloc(ArenaValue::Array(&[]))),
        ResolvedInput::Bridge(av) => {
            return map_arena_bridge(av, &args[1], actx, context, engine, arena);
        }
    };

    let len = src.len();
    if len == 0 {
        return Ok(arena.alloc(ArenaValue::Array(&[])));
    }

    // BODY FAST PATH: var with simple shape — identity (empty segments) or
    // field extract. Both emit InputRef per item with zero per-iteration allocs.
    if let CompiledNode::CompiledVar {
        scope_level: 0,
        segments,
        reduce_hint: ReduceHint::None,
        metadata_hint: MetadataHint::None,
        default_value: None,
        ..
    } = body
    {
        let mut results: bumpalo::collections::Vec<'a, ArenaValue<'a>> =
            bumpalo::collections::Vec::with_capacity_in(len, arena);
        if segments.is_empty() {
            for i in 0..len {
                results.push(ArenaValue::InputRef(src.get(i)));
            }
        } else {
            for i in 0..len {
                let item = src.get(i);
                match super::variable::try_traverse_segments(item, segments) {
                    Some(v) => results.push(ArenaValue::InputRef(v)),
                    None => results.push(ArenaValue::Null),
                }
            }
        }
        return Ok(arena.alloc(ArenaValue::Array(results.into_bump_slice())));
    }

    // GENERAL PATH: zero-clone via ArenaContextStack — frame data is
    // `InputRef(item)`; body dispatches through arena.
    let mut results: bumpalo::collections::Vec<'a, ArenaValue<'a>> =
        bumpalo::collections::Vec::with_capacity_in(len, arena);
    let mut pushed = false;
    for i in 0..len {
        let item = src.get(i);
        let item_av: &'a ArenaValue<'a> = arena.alloc(ArenaValue::InputRef(item));
        if !pushed {
            actx.push_with_index(item_av, 0);
            pushed = true;
        } else {
            actx.replace_top_data(item_av, i);
        }
        let av = engine.evaluate_arena_node(body, actx, context, arena)?;
        results.push(crate::arena::value::reborrow_arena_value(av));
    }
    if pushed {
        actx.pop();
    }
    Ok(arena.alloc(ArenaValue::Array(results.into_bump_slice())))
}

/// Internal helper: arena-mode quantifier (all / some / none).
/// `early_truthy` controls short-circuit semantics:
///   - `all`: early_truthy = false (false ⇒ return false immediately)
///   - `some`: early_truthy = true (true ⇒ return true immediately)
///   - `none`: same as `some` but invert the final result
#[inline]
#[allow(clippy::too_many_arguments)] // 5 contextual + 3 quantifier-shape flags; bundling the flags
                                     // into a struct adds noise without simplifying the call sites.
fn evaluate_quantifier_arena<'a>(
    args: &[CompiledNode],
    actx: &mut ArenaContextStack<'a>,
    context: &mut ContextStack,
    engine: &DataLogic,
    arena: &'a Bump,
    short_circuit_on: bool,
    invert_final: bool,
    empty_result: bool,
) -> Result<&'a ArenaValue<'a>> {
    if args.len() != 2 {
        return Err(Error::InvalidArguments(INVALID_ARGS.into()));
    }

    let predicate = &args[1];
    let src = match resolve_iter_input(&args[0], actx, context, engine, arena)? {
        ResolvedInput::Iterable(s) => s,
        ResolvedInput::Empty => return Ok(arena.alloc(ArenaValue::Bool(empty_result))),
        ResolvedInput::Bridge(av) => {
            return quantifier_arena_bridge(
                av,
                predicate,
                short_circuit_on,
                invert_final,
                empty_result,
                actx,
                context,
                engine,
                arena,
            );
        }
    };

    if src.is_empty() {
        return Ok(arena.alloc(ArenaValue::Bool(empty_result)));
    }

    // Fast predicate path — no context push, no clones.
    if let Some(fast_pred) = FastPredicate::try_detect(predicate) {
        let len = src.len();
        for i in 0..len {
            if fast_pred.evaluate(src.get(i)) == short_circuit_on {
                let result = if invert_final {
                    !short_circuit_on
                } else {
                    short_circuit_on
                };
                return Ok(arena.alloc(ArenaValue::Bool(result)));
            }
        }
        let result = if invert_final {
            short_circuit_on
        } else {
            !short_circuit_on
        };
        return Ok(arena.alloc(ArenaValue::Bool(result)));
    }

    // General path: zero-clone via ArenaContextStack.
    let mut pushed = false;
    let mut found_short = false;
    let len = src.len();
    for i in 0..len {
        let item = src.get(i);
        let item_av: &'a ArenaValue<'a> = arena.alloc(ArenaValue::InputRef(item));
        if !pushed {
            actx.push_with_index(item_av, 0);
            pushed = true;
        } else {
            actx.replace_top_data(item_av, i);
        }
        let av = engine.evaluate_arena_node(predicate, actx, context, arena)?;
        if crate::arena::is_truthy_arena(av, engine) == short_circuit_on {
            found_short = true;
            break;
        }
    }
    if pushed {
        actx.pop();
    }

    let result = if found_short {
        if invert_final {
            !short_circuit_on
        } else {
            short_circuit_on
        }
    } else if invert_final {
        short_circuit_on
    } else {
        !short_circuit_on
    };
    Ok(arena.alloc(ArenaValue::Bool(result)))
}

/// Bridge a quantifier op to its value-mode implementation. Used when the
/// arena resolver can't iterate the input (e.g. computed non-array, object).
fn bridge_quantifier_value_mode(
    args: &[CompiledNode],
    context: &mut ContextStack,
    engine: &DataLogic,
    short_circuit_on: bool,
    invert_final: bool,
) -> Result<Value> {
    use crate::eval_mode::Plain;
    // Map (short_circuit_on, invert_final) back to which quantifier this is.
    // (false, false) = all
    // (true,  false) = some
    // (true,  true)  = none
    if !short_circuit_on {
        evaluate_all::<Plain>(args, context, engine, &mut Plain)
    } else if invert_final {
        evaluate_none::<Plain>(args, context, engine, &mut Plain)
    } else {
        evaluate_some::<Plain>(args, context, engine, &mut Plain)
    }
}

/// Arena-mode `all` — true iff every item satisfies predicate. Short-circuits on false.
#[inline]
pub(crate) fn evaluate_all_arena<'a>(
    args: &[CompiledNode],
    actx: &mut ArenaContextStack<'a>,
    context: &mut ContextStack,
    engine: &DataLogic,
    arena: &'a Bump,
) -> Result<&'a ArenaValue<'a>> {
    // all: early-exit on false; empty array ⇒ false (matching existing impl,
    // which deliberately rejects vacuous truth — see evaluate_all in this file).
    evaluate_quantifier_arena(args, actx, context, engine, arena, false, false, false)
}

/// Arena-mode `some` — true iff any item satisfies predicate. Short-circuits on true.
#[inline]
pub(crate) fn evaluate_some_arena<'a>(
    args: &[CompiledNode],
    actx: &mut ArenaContextStack<'a>,
    context: &mut ContextStack,
    engine: &DataLogic,
    arena: &'a Bump,
) -> Result<&'a ArenaValue<'a>> {
    // some: early-exit on true; empty array ⇒ false.
    evaluate_quantifier_arena(args, actx, context, engine, arena, true, false, false)
}

/// Arena-mode `none` — true iff no item satisfies predicate. Short-circuits on true.
#[inline]
pub(crate) fn evaluate_none_arena<'a>(
    args: &[CompiledNode],
    actx: &mut ArenaContextStack<'a>,
    context: &mut ContextStack,
    engine: &DataLogic,
    arena: &'a Bump,
) -> Result<&'a ArenaValue<'a>> {
    // none: early-exit on true (then return false); empty array ⇒ true.
    evaluate_quantifier_arena(args, actx, context, engine, arena, true, true, true)
}

/// Arena-mode `reduce` — folds an array into a single value via accumulator.
/// Arena-mode `reduce`. Phase 6.4: input via resolve_iter_input (so
/// `reduce(filter(...), +, 0)` composes), and inline arithmetic fast paths
/// for the dominant `current op accumulator` pattern.
#[inline]
pub(crate) fn evaluate_reduce_arena<'a>(
    args: &[CompiledNode],
    actx: &mut ArenaContextStack<'a>,
    context: &mut ContextStack,
    engine: &DataLogic,
    arena: &'a Bump,
) -> Result<&'a ArenaValue<'a>> {
    if args.len() < 2 || args.len() > 3 {
        return Err(Error::InvalidArguments(INVALID_ARGS.into()));
    }

    let body = &args[1];
    let initial = if args.len() == 3 {
        engine.evaluate_node(&args[2], context)?
    } else {
        Value::Null
    };

    let src = match resolve_iter_input(&args[0], actx, context, engine, arena)? {
        ResolvedInput::Iterable(s) => s,
        ResolvedInput::Empty => return Ok(arena.alloc(value_to_arena(&initial, arena))),
        ResolvedInput::Bridge(av) => {
            return reduce_arena_bridge(av, body, &initial, actx, context, engine, arena);
        }
    };

    if src.is_empty() {
        return Ok(arena.alloc(value_to_arena(&initial, arena)));
    }

    // FAST PATH: {op: [val("current"[+path]), val("accumulator")]} for + / - / *.
    // Mirrors try_reduce_fast_path but iterates IterSrc to support both
    // borrowed-input and arena-Refs cases.
    if let CompiledNode::BuiltinOperator {
        opcode,
        args: body_args,
        ..
    } = body
        && body_args.len() == 2
        && matches!(opcode, OpCode::Add | OpCode::Multiply | OpCode::Subtract)
        && let Some(result) = try_reduce_fast_path_arena(&src, &initial, body_args, *opcode)
    {
        return Ok(arena.alloc(value_to_arena(&result, arena)));
    }

    // GENERAL PATH: zero-clone via ArenaContextStack. Frame holds
    // `&'a ArenaValue<'a>` for both the current item and the accumulator.
    // Body dispatches through arena and the result threads as
    // `&'a ArenaValue<'a>` between iterations.
    let mut acc_av: &'a ArenaValue<'a> = arena.alloc(value_to_arena(&initial, arena));
    let mut pushed = false;
    let len = src.len();
    for i in 0..len {
        let item = src.get(i);
        let item_av: &'a ArenaValue<'a> = arena.alloc(ArenaValue::InputRef(item));
        if !pushed {
            actx.push_reduce(item_av, acc_av);
            pushed = true;
        } else {
            actx.replace_reduce_data(item_av, acc_av);
        }
        let _ = i; // silence unused
        acc_av = engine.evaluate_arena_node(body, actx, context, arena)?;
    }
    if pushed {
        actx.pop();
    }
    Ok(acc_av)
}

/// Arena variant of `try_reduce_fast_path` — same logic, iterates `IterSrc`.
fn try_reduce_fast_path_arena(
    src: &IterSrc<'_>,
    initial: &Value,
    body_args: &[CompiledNode],
    opcode: OpCode,
) -> Option<Value> {
    // Identify which arg is current and which is accumulator.
    let (current_arg, _acc_arg) = match (&body_args[0], &body_args[1]) {
        (
            CompiledNode::CompiledVar { reduce_hint: hint0, .. },
            CompiledNode::CompiledVar { reduce_hint: hint1, .. },
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

    let current_segments = if let CompiledNode::CompiledVar { segments, reduce_hint, .. } =
        current_arg
    {
        match reduce_hint {
            ReduceHint::Current => &[][..],
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

    let len = src.len();

    // Integer fast path.
    let mut acc_i = initial.as_i64();
    if acc_i.is_some() {
        let mut all_int = true;
        for i in 0..len {
            let item = src.get(i);
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

    // f64 fallback.
    let mut acc_f = initial.as_f64()?;
    for i in 0..len {
        let item = src.get(i);
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

/// Arena-mode `merge`. Flattens its args (each may itself be a nested arena
/// op) into a single array, skipping nulls. Returns a slice of `InputRef`s
/// pointing at the original Values — no per-element clones.
#[inline]
pub(crate) fn evaluate_merge_arena<'a>(
    args: &[CompiledNode],
    actx: &mut ArenaContextStack<'a>,
    context: &mut ContextStack,
    engine: &DataLogic,
    arena: &'a Bump,
) -> Result<&'a ArenaValue<'a>> {
    let mut results: bumpalo::collections::Vec<'a, ArenaValue<'a>> =
        bumpalo::collections::Vec::new_in(arena);

    for arg in args {
        let av = engine.evaluate_arena_node(arg, actx, context, arena)?;
        match av {
            // Direct arena Array (e.g. result of upstream arena filter/map).
            ArenaValue::Array(items) => {
                for item in items.iter() {
                    if !item_is_null(item) {
                        // Each item is typically already an InputRef; extract
                        // the underlying &Value when possible to keep the
                        // result uniformly InputRef-shaped for downstream
                        // consumers.
                        match item {
                            ArenaValue::InputRef(v) => {
                                if !v.is_null() {
                                    results.push(ArenaValue::InputRef(v));
                                }
                            }
                            _ => {
                                // Computed arena value (Number/Bool/String).
                                // Cheap to copy the enum reference into our
                                // result slice.
                                results.push(reborrow_arena(item));
                            }
                        }
                    }
                }
            }
            // Borrowed input array — iterate, push InputRef per non-null.
            ArenaValue::InputRef(Value::Array(arr)) => {
                for item in arr.iter() {
                    if !item.is_null() {
                        results.push(ArenaValue::InputRef(item));
                    }
                }
            }
            // Null inputs are skipped per merge semantics.
            ArenaValue::InputRef(Value::Null) | ArenaValue::Null => {}
            // Scalar / object — push as-is.
            other => results.push(reborrow_arena(other)),
        }
    }

    Ok(arena.alloc(ArenaValue::Array(results.into_bump_slice())))
}

/// Cheap shallow copy of an `ArenaValue` enum (clones the discriminant +
/// inline payload bytes — no heap traffic). Used by merge to copy non-Array
/// items into its result slice without allocating.
#[inline]
fn reborrow_arena<'a>(av: &ArenaValue<'a>) -> ArenaValue<'a> {
    match av {
        ArenaValue::Null => ArenaValue::Null,
        ArenaValue::Bool(b) => ArenaValue::Bool(*b),
        ArenaValue::Number(n) => ArenaValue::Number(*n),
        ArenaValue::String(s) => ArenaValue::String(s),
        ArenaValue::Array(items) => ArenaValue::Array(items),
        ArenaValue::Object(pairs) => ArenaValue::Object(pairs),
        #[cfg(feature = "datetime")]
        ArenaValue::DateTime(dt) => ArenaValue::DateTime(dt.clone()),
        #[cfg(feature = "datetime")]
        ArenaValue::Duration(d) => ArenaValue::Duration(d.clone()),
        ArenaValue::InputRef(v) => ArenaValue::InputRef(v),
    }
}

/// True iff this arena value would be `null` after conversion to `Value`.
#[inline]
fn item_is_null(av: &ArenaValue<'_>) -> bool {
    matches!(av, ArenaValue::Null) || matches!(av, ArenaValue::InputRef(Value::Null))
}
