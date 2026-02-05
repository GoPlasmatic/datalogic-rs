//! Error handling operator for graceful failure recovery.
//!
//! The `try` operator provides exception-like error handling in JSONLogic expressions.
//! It evaluates arguments in sequence until one succeeds, similar to try-catch in
//! traditional programming languages.
//!
//! # Syntax
//!
//! ```json
//! {"try": [expression, fallback1, fallback2, ...]}
//! ```
//!
//! # Behavior
//!
//! 1. Evaluates each argument in order until one succeeds
//! 2. Returns the result of the first successful evaluation
//! 3. If all arguments fail, returns the last error
//! 4. The final argument can access error context via `{"var": ""}` when catching
//!    a `throw` error
//!
//! # Error Context
//!
//! When catching a thrown error, the last fallback argument receives the error
//! object as its context, allowing error inspection:
//!
//! ```json
//! {"try": [
//!   {"throw": {"code": 404, "message": "Not found"}},
//!   {"cat": ["Error: ", {"var": "message"}]}
//! ]}
//! // Returns: "Error: Not found"
//! ```
//!
//! # Related
//!
//! - [`throw`](super::throw) - Throw an error to be caught by `try`

use serde_json::Value;

use crate::{CompiledNode, ContextStack, DataLogic, Error, Result};
use std::collections::HashMap;

/// Evaluate the last argument of try with error context if applicable.
/// Uses `take()` to move the error object instead of cloning.
#[inline(always)]
fn try_last_with_error_context(
    arg: &CompiledNode,
    last_error: &mut Option<Error>,
    context: &mut ContextStack,
    engine: &DataLogic,
) -> Result<Value> {
    if let Some(Error::Thrown(error_obj)) = last_error.take() {
        context.push(error_obj);
        let result = engine.evaluate_node(arg, context);
        context.pop();
        result
    } else {
        engine.evaluate_node(arg, context)
    }
}

/// Try operator function - catches errors and provides fallback values
#[inline]
pub fn evaluate_try(
    args: &[CompiledNode],
    context: &mut ContextStack,
    engine: &DataLogic,
) -> Result<Value> {
    if args.is_empty() {
        return Err(Error::InvalidArguments(
            "try requires at least one argument".to_string(),
        ));
    }

    // Fast path: single argument — just evaluate it
    if args.len() == 1 {
        return engine.evaluate_node(&args[0], context);
    }

    // Fast path: two arguments (most common pattern)
    if args.len() == 2 {
        match engine.evaluate_node(&args[0], context) {
            Ok(result) => return Ok(result),
            Err(err) => {
                let mut last_error = Some(err);
                return try_last_with_error_context(&args[1], &mut last_error, context, engine);
            }
        }
    }

    // General path: 3+ arguments
    let mut last_error: Option<Error> = None;
    let last_idx = args.len() - 1;

    for (i, arg) in args.iter().enumerate() {
        if i == last_idx {
            return try_last_with_error_context(arg, &mut last_error, context, engine);
        }
        match engine.evaluate_node(arg, context) {
            Ok(result) => return Ok(result),
            Err(err) => last_error = Some(err),
        }
    }

    match last_error {
        Some(err) => Err(err),
        None => Err(Error::InvalidArguments(
            "try: no arguments provided".to_string(),
        )),
    }
}

/// Traced version of try - evaluates arguments with tracing for step-by-step debugging
#[inline]
pub fn evaluate_try_traced(
    args: &[CompiledNode],
    context: &mut ContextStack,
    engine: &DataLogic,
    collector: &mut crate::trace::TraceCollector,
    node_id_map: &HashMap<usize, u32>,
) -> Result<Value> {
    if args.is_empty() {
        return Err(Error::InvalidArguments(
            "try requires at least one argument".to_string(),
        ));
    }

    let mut last_error: Option<Error> = None;

    for (i, arg) in args.iter().enumerate() {
        if i == args.len() - 1 && i > 0 {
            if let Some(Error::Thrown(error_obj)) = last_error.take() {
                context.push(error_obj);
                match engine.evaluate_node_traced(arg, context, collector, node_id_map) {
                    Ok(result) => {
                        context.pop();
                        return Ok(result);
                    }
                    Err(new_err) => {
                        context.pop();
                        last_error = Some(new_err);
                    }
                }
            } else {
                match engine.evaluate_node_traced(arg, context, collector, node_id_map) {
                    Ok(result) => return Ok(result),
                    Err(err) => last_error = Some(err),
                }
            }
        } else {
            match engine.evaluate_node_traced(arg, context, collector, node_id_map) {
                Ok(result) => return Ok(result),
                Err(err) => last_error = Some(err),
            }
        }
    }

    match last_error {
        Some(err) => Err(err),
        None => Err(Error::InvalidArguments(
            "try: no arguments provided".to_string(),
        )),
    }
}
