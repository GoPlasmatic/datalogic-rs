use datavalue::OwnedDataValue;

use crate::arena::{ContextStack, DataValue};
use crate::{CompiledNode, Engine, Error, Result};
use bumpalo::Bump;

/// `throw`. Builds the error object directly from the argument's arena form.
#[inline]
pub(crate) fn evaluate_throw<'a>(
    args: &'a [CompiledNode],
    ctx: &mut ContextStack<'a>,
    engine: &Engine,
    arena: &'a Bump,
) -> Result<&'a DataValue<'a>> {
    let owned: OwnedDataValue = if args.is_empty() {
        OwnedDataValue::Null
    } else if let CompiledNode::Value { value, .. } = &args[0] {
        // Literal fast path — skip arena dispatch.
        value.clone()
    } else {
        let av = engine.evaluate_node(&args[0], ctx, arena)?;
        av.to_owned()
    };

    let owned = match owned {
        OwnedDataValue::Object(_) => owned,
        OwnedDataValue::String(s) => OwnedDataValue::object([("type", s)]),
        other => OwnedDataValue::object([("type", format!("{:?}", other))]),
    };

    Err(Error::thrown(owned))
}
