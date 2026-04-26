use serde_json::Value;

use crate::arena::{ArenaContextStack, ArenaValue, arena_to_value};
use crate::{CompiledNode, DataLogic, Error, Result};
use bumpalo::Bump;

/// `throw`. Builds the error object directly from the argument's arena form.
#[inline]
pub(crate) fn evaluate_throw_arena<'a>(
    args: &'a [CompiledNode],
    actx: &mut ArenaContextStack<'a>,
    engine: &DataLogic,
    arena: &'a Bump,
) -> Result<&'a ArenaValue<'a>> {
    let error_obj: Value = if args.is_empty() {
        Value::Null
    } else if let CompiledNode::Value { value, .. } = &args[0] {
        // Literal fast path — skip arena dispatch.
        value.clone()
    } else {
        let av = engine.evaluate_arena_node(&args[0], actx, arena)?;
        arena_to_value(av)
    };

    let error_obj = match error_obj {
        Value::Object(_) => error_obj,
        Value::String(s) => serde_json::json!({"type": s}),
        other => serde_json::json!({"type": other.to_string()}),
    };

    Err(Error::Thrown(error_obj))
}
