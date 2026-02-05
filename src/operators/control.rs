use serde_json::Value;
use std::collections::HashMap;

use super::helpers::{check_invalid_args_marker, is_truthy};
use crate::trace::TraceCollector;
use crate::{CompiledNode, ContextStack, DataLogic, Result};

/// If operator function - supports if/then/else and if/elseif/else chains
#[inline]
pub fn evaluate_if(
    args: &[CompiledNode],
    context: &mut ContextStack,
    engine: &DataLogic,
) -> Result<Value> {
    if args.is_empty() {
        return Ok(Value::Null);
    }

    check_invalid_args_marker(args)?;

    // Support variadic if/elseif/else chains
    let mut i = 0;
    while i < args.len() {
        if i == args.len() - 1 {
            // Final else clause
            return engine.evaluate_node(&args[i], context);
        }

        // Evaluate condition using Cow to avoid cloning literals
        let condition = engine.evaluate_node_cow(&args[i], context)?;
        if is_truthy(&condition, engine) {
            // Evaluate then branch
            if i + 1 < args.len() {
                return engine.evaluate_node(&args[i + 1], context);
            } else {
                return Ok(condition.into_owned());
            }
        }

        // Move to next if/elseif pair
        i += 2;
    }

    Ok(Value::Null)
}

/// Ternary operator function (?:)
#[inline]
pub fn evaluate_ternary(
    args: &[CompiledNode],
    context: &mut ContextStack,
    engine: &DataLogic,
) -> Result<Value> {
    if args.len() < 3 {
        return Ok(Value::Null);
    }

    let condition = engine.evaluate_node_cow(&args[0], context)?;

    if is_truthy(&condition, engine) {
        engine.evaluate_node(&args[1], context)
    } else {
        engine.evaluate_node(&args[2], context)
    }
}

/// Coalesce operator function (??) - returns first non-null value
#[inline]
pub fn evaluate_coalesce(
    args: &[CompiledNode],
    context: &mut ContextStack,
    engine: &DataLogic,
) -> Result<Value> {
    // Empty args returns null
    if args.is_empty() {
        return Ok(Value::Null);
    }

    // Return the first non-null value
    for arg in args {
        let value = engine.evaluate_node_cow(arg, context)?;
        if *value != Value::Null {
            return Ok(value.into_owned());
        }
    }

    // All values were null
    Ok(Value::Null)
}

/// Switch/match operator - evaluates discriminant once and matches against case pairs
#[inline]
pub fn evaluate_switch(
    args: &[CompiledNode],
    context: &mut ContextStack,
    engine: &DataLogic,
) -> Result<Value> {
    // args[0] = discriminant, args[1] = cases array, args[2] = optional default
    if args.len() < 2 {
        return Ok(Value::Null);
    }

    // Evaluate discriminant once
    let discriminant = engine.evaluate_node(&args[0], context)?;

    // Cases should be a CompiledNode::Array of [match_value, result] pairs
    // or a CompiledNode::Value containing a pre-evaluated array (from static optimization)
    match &args[1] {
        CompiledNode::Array { nodes } => {
            for case_node in nodes.iter() {
                match case_node {
                    CompiledNode::Array { nodes: pair } if pair.len() >= 2 => {
                        let case_value = engine.evaluate_node(&pair[0], context)?;
                        if discriminant == case_value {
                            return engine.evaluate_node(&pair[1], context);
                        }
                    }
                    CompiledNode::Value {
                        value: Value::Array(pair),
                    } if pair.len() >= 2 => {
                        // Static-optimized pair
                        if discriminant == pair[0] {
                            return Ok(pair[1].clone());
                        }
                    }
                    _ => {}
                }
            }
        }
        CompiledNode::Value {
            value: Value::Array(cases),
        } => {
            // Entire cases array was statically evaluated
            for case in cases {
                if let Value::Array(pair) = case {
                    if pair.len() >= 2 && discriminant == pair[0] {
                        return Ok(pair[1].clone());
                    }
                }
            }
        }
        _ => {}
    }

    // No match found - evaluate default if present
    if args.len() > 2 {
        return engine.evaluate_node(&args[2], context);
    }

    Ok(Value::Null)
}

// ============================================================================
// Traced versions of control flow operators
// ============================================================================

/// Traced version of `if` operator - only evaluates the selected branch.
#[inline]
pub fn evaluate_if_traced(
    args: &[CompiledNode],
    context: &mut ContextStack,
    engine: &DataLogic,
    collector: &mut TraceCollector,
    node_id_map: &HashMap<usize, u32>,
) -> Result<Value> {
    if args.is_empty() {
        return Ok(Value::Null);
    }

    check_invalid_args_marker(args)?;

    // Support variadic if/elseif/else chains
    let mut i = 0;
    while i < args.len() {
        if i == args.len() - 1 {
            // Final else clause
            return engine.evaluate_node_traced(&args[i], context, collector, node_id_map);
        }

        // Evaluate condition
        let condition = engine.evaluate_node_traced(&args[i], context, collector, node_id_map)?;
        if is_truthy(&condition, engine) {
            // Evaluate then branch (only this branch gets traced)
            if i + 1 < args.len() {
                return engine.evaluate_node_traced(&args[i + 1], context, collector, node_id_map);
            } else {
                return Ok(condition);
            }
        }

        // Move to next if/elseif pair
        i += 2;
    }

    Ok(Value::Null)
}

/// Traced version of `ternary` (?:) operator - only evaluates the selected branch.
#[inline]
pub fn evaluate_ternary_traced(
    args: &[CompiledNode],
    context: &mut ContextStack,
    engine: &DataLogic,
    collector: &mut TraceCollector,
    node_id_map: &HashMap<usize, u32>,
) -> Result<Value> {
    if args.len() < 3 {
        return Ok(Value::Null);
    }

    let condition = engine.evaluate_node_traced(&args[0], context, collector, node_id_map)?;

    if is_truthy(&condition, engine) {
        // Only evaluate the then branch
        engine.evaluate_node_traced(&args[1], context, collector, node_id_map)
    } else {
        // Only evaluate the else branch
        engine.evaluate_node_traced(&args[2], context, collector, node_id_map)
    }
}

/// Traced version of `coalesce` (??) operator - only evaluates until first non-null.
#[inline]
pub fn evaluate_coalesce_traced(
    args: &[CompiledNode],
    context: &mut ContextStack,
    engine: &DataLogic,
    collector: &mut TraceCollector,
    node_id_map: &HashMap<usize, u32>,
) -> Result<Value> {
    // Empty args returns null
    if args.is_empty() {
        return Ok(Value::Null);
    }

    // Return the first non-null value (short-circuit)
    for arg in args {
        let value = engine.evaluate_node_traced(arg, context, collector, node_id_map)?;
        if value != Value::Null {
            return Ok(value);
        }
    }

    // All values were null
    Ok(Value::Null)
}

/// Traced version of `switch`/`match` operator - only evaluates matched result.
#[inline]
pub fn evaluate_switch_traced(
    args: &[CompiledNode],
    context: &mut ContextStack,
    engine: &DataLogic,
    collector: &mut TraceCollector,
    node_id_map: &HashMap<usize, u32>,
) -> Result<Value> {
    if args.len() < 2 {
        return Ok(Value::Null);
    }

    // Evaluate discriminant once with tracing
    let discriminant = engine.evaluate_node_traced(&args[0], context, collector, node_id_map)?;

    match &args[1] {
        CompiledNode::Array { nodes } => {
            for case_node in nodes.iter() {
                match case_node {
                    CompiledNode::Array { nodes: pair } if pair.len() >= 2 => {
                        let case_value = engine.evaluate_node_traced(
                            &pair[0],
                            context,
                            collector,
                            node_id_map,
                        )?;
                        if discriminant == case_value {
                            return engine.evaluate_node_traced(
                                &pair[1],
                                context,
                                collector,
                                node_id_map,
                            );
                        }
                    }
                    CompiledNode::Value {
                        value: Value::Array(pair),
                    } if pair.len() >= 2 => {
                        if discriminant == pair[0] {
                            return Ok(pair[1].clone());
                        }
                    }
                    _ => {}
                }
            }
        }
        CompiledNode::Value {
            value: Value::Array(cases),
        } => {
            for case in cases {
                if let Value::Array(pair) = case {
                    if pair.len() >= 2 && discriminant == pair[0] {
                        return Ok(pair[1].clone());
                    }
                }
            }
        }
        _ => {}
    }

    // No match found - evaluate default if present
    if args.len() > 2 {
        return engine.evaluate_node_traced(&args[2], context, collector, node_id_map);
    }

    Ok(Value::Null)
}
