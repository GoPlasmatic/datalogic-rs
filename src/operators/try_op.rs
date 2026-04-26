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

use crate::arena::{ArenaContextStack, ArenaValue};
use crate::{CompiledNode, DataLogic, Error, Result};
use bumpalo::Bump;

#[inline]
pub(crate) fn evaluate_try_arena<'a>(
    args: &'a [CompiledNode],
    actx: &mut ArenaContextStack<'a>,
    engine: &DataLogic,
    arena: &'a Bump,
) -> Result<&'a ArenaValue<'a>> {
    if args.is_empty() {
        return Ok(arena.alloc(ArenaValue::Null));
    }
    if args.len() == 1 {
        return engine.evaluate_arena_node(&args[0], actx, arena);
    }

    // Multi-arg form: try arms in sequence; final arm receives the error
    // object as its context.
    let last_idx = args.len() - 1;
    let mut last_err: Option<Error> = None;
    for (i, arg) in args.iter().enumerate() {
        if i == last_idx {
            return arena_try_last_with_error_context(arg, &mut last_err, actx, engine, arena);
        }
        let saved_len = actx.error_path_len();
        match engine.evaluate_arena_node(arg, actx, arena) {
            Ok(v) => return Ok(v),
            Err(e) => {
                actx.truncate_error_path(saved_len);
                last_err = Some(e);
            }
        }
    }
    Err(last_err.unwrap_or_else(|| Error::InvalidArguments(crate::constants::INVALID_ARGS.into())))
}

/// Pushes the thrown error object onto the arena context stack as the
/// current frame so the catch arm's `var`/`val` lookups see error fields.
#[inline]
fn arena_try_last_with_error_context<'a>(
    arg: &'a CompiledNode,
    last_error: &mut Option<Error>,
    actx: &mut ArenaContextStack<'a>,
    engine: &DataLogic,
    arena: &'a Bump,
) -> Result<&'a ArenaValue<'a>> {
    if let Some(Error::Thrown(error_obj)) = last_error.take() {
        let av: &'a ArenaValue<'a> = arena.alloc(crate::arena::value_to_arena(&error_obj, arena));
        actx.push(av);
        let result = engine.evaluate_arena_node(arg, actx, arena);
        actx.pop();
        result
    } else {
        engine.evaluate_arena_node(arg, actx, arena)
    }
}
