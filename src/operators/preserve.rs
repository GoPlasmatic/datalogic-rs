//! Structure preservation operator for templating mode.
//!
//! The `preserve` operator is used when `preserve_structure` mode is enabled on the engine.
//! It allows literal values to pass through without being interpreted as operators,
//! enabling JSON templating where the output structure mirrors the input.
//!
//! # Use Case
//!
//! When processing JSON templates, you may want some object keys to be treated as
//! literal output fields rather than operators. The `preserve` operator marks these
//! values for pass-through.
//!
//! # Behavior
//!
//! - With no arguments: returns an empty array
//! - With one argument: evaluates and returns that argument
//! - With multiple arguments: returns an array of evaluated arguments
//!
//! # Example
//!
//! ```json
//! // With preserve_structure enabled, unknown keys become output fields
//! {
//!   "name": {"var": "user.name"},
//!   "status": "active"
//! }
//! // Output: {"name": "John", "status": "active"}
//! ```

use crate::arena::{ArenaContextStack, ArenaValue, value::reborrow_arena_value};
use crate::{CompiledNode, DataLogic, Result};
use bumpalo::Bump;

/// Native arena-mode `preserve`.
/// - 0 args: empty array (singleton).
/// - 1 arg : the evaluated arg (or its literal value, fast-path).
/// - N args: arena array of evaluated args.
#[inline]
pub(crate) fn evaluate_preserve_arena<'a>(
    args: &'a [CompiledNode],
    actx: &mut ArenaContextStack<'a>,
    engine: &DataLogic,
    arena: &'a Bump,
) -> Result<&'a ArenaValue<'a>> {
    match args.len() {
        0 => Ok(crate::arena::pool::singleton_empty_array()),
        1 => {
            // Literal fast path — skip evaluate_arena_node dispatch.
            if let CompiledNode::Value { value, .. } = &args[0] {
                return Ok(arena.alloc(crate::arena::value_to_arena(value, arena)));
            }
            engine.evaluate_arena_node(&args[0], actx, arena)
        }
        _ => {
            let mut items: bumpalo::collections::Vec<'a, ArenaValue<'a>> =
                bumpalo::collections::Vec::with_capacity_in(args.len(), arena);
            for arg in args {
                let av = engine.evaluate_arena_node(arg, actx, arena)?;
                items.push(reborrow_arena_value(av));
            }
            Ok(arena.alloc(ArenaValue::Array(items.into_bump_slice())))
        }
    }
}
