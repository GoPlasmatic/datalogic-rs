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

use crate::arena::{ContextStack, DataValue};
use crate::{CompiledNode, Engine, Error, Result};
use bumpalo::Bump;

#[inline]
pub(crate) fn evaluate_try<'a>(
    args: &'a [CompiledNode],
    ctx: &mut ContextStack<'a>,
    engine: &Engine,
    arena: &'a Bump,
) -> Result<&'a DataValue<'a>> {
    if args.is_empty() {
        return Ok(crate::arena::pool::singleton_null());
    }
    if args.len() == 1 {
        return engine.dispatch_node(&args[0], ctx, arena);
    }

    // Multi-arg form: try arms in sequence; final arm receives the error
    // object as its context.
    let last_idx = args.len() - 1;
    let mut last_err: Option<Error> = None;
    for (i, arg) in args.iter().enumerate() {
        if i == last_idx {
            return try_last_with_error_context(arg, &mut last_err, ctx, engine, arena);
        }
        let saved_len = ctx.error_path_len();
        match engine.dispatch_node(arg, ctx, arena) {
            Ok(v) => return Ok(v),
            Err(e) => {
                ctx.truncate_error_path(saved_len);
                last_err = Some(e);
            }
        }
    }
    Err(last_err.unwrap_or_else(|| Error::invalid_arguments(crate::constants::INVALID_ARGS)))
}

/// Pushes the thrown error object onto the arena context stack as the
/// current frame so the catch arm's `var`/`val` lookups see error fields.
#[inline]
fn try_last_with_error_context<'a>(
    arg: &'a CompiledNode,
    last_error: &mut Option<Error>,
    ctx: &mut ContextStack<'a>,
    engine: &Engine,
    arena: &'a Bump,
) -> Result<&'a DataValue<'a>> {
    if let Some(Error {
        kind: crate::ErrorKind::Thrown(error_obj),
        ..
    }) = last_error.take()
    {
        let av: &'a DataValue<'a> = arena.alloc(error_obj.to_arena(arena));
        ctx.push(av);
        let result = engine.dispatch_node(arg, ctx, arena);
        ctx.pop();
        result
    } else {
        engine.dispatch_node(arg, ctx, arena)
    }
}
