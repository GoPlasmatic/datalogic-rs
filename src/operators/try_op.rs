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

use crate::eval_mode::Mode;
use crate::{CompiledNode, ContextStack, DataLogic, Error, Result};

/// Evaluate the last argument of try with error context if applicable.
/// Uses `take()` to move the error object instead of cloning.
#[inline]
fn try_last_with_error_context<M: Mode>(
    arg: &CompiledNode,
    last_error: &mut Option<Error>,
    context: &mut ContextStack,
    engine: &DataLogic,
    mode: &mut M,
) -> Result<Value> {
    if let Some(Error::Thrown(error_obj)) = last_error.take() {
        context.push(error_obj);
        let result = engine.evaluate_node_with_mode::<M>(arg, context, mode);
        context.pop();
        result
    } else {
        engine.evaluate_node_with_mode::<M>(arg, context, mode)
    }
}

/// Try operator — catches errors and falls back through alternative arguments.
///
/// Generic over [`Mode`] so plain and traced dispatch share the same body.
/// Snapshots the error breadcrumb length on entry and truncates back to it
/// whenever a catch succeeds, so the final breadcrumb only reflects the
/// error that escapes `try` (if any) — not every arm that was tried and
/// swallowed along the way.
#[inline]
pub fn evaluate_try<M: Mode>(
    args: &[CompiledNode],
    context: &mut ContextStack,
    engine: &DataLogic,
    mode: &mut M,
) -> Result<Value> {
    if args.is_empty() {
        return Err(Error::InvalidArguments(
            "try requires at least one argument".to_string(),
        ));
    }

    // Fast path: single argument — just evaluate it
    if args.len() == 1 {
        return engine.evaluate_node_with_mode::<M>(&args[0], context, mode);
    }

    // Fast path: two arguments (most common pattern)
    if args.len() == 2 {
        let checkpoint = context.error_path_len();
        match engine.evaluate_node_with_mode::<M>(&args[0], context, mode) {
            Ok(result) => return Ok(result),
            Err(err) => {
                // Swallowed error — drop any breadcrumb it accumulated.
                context.truncate_error_path(checkpoint);
                let mut last_error = Some(err);
                return try_last_with_error_context::<M>(
                    &args[1],
                    &mut last_error,
                    context,
                    engine,
                    mode,
                );
            }
        }
    }

    // General path: 3+ arguments
    let mut last_error: Option<Error> = None;
    let last_idx = args.len() - 1;

    for (i, arg) in args.iter().enumerate() {
        if i == last_idx {
            return try_last_with_error_context::<M>(
                arg,
                &mut last_error,
                context,
                engine,
                mode,
            );
        }
        let checkpoint = context.error_path_len();
        match engine.evaluate_node_with_mode::<M>(arg, context, mode) {
            Ok(result) => return Ok(result),
            Err(err) => {
                context.truncate_error_path(checkpoint);
                last_error = Some(err);
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
