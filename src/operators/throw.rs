use datavalue::OwnedDataValue;

use crate::arena::{DataContextStack, DataValue};
use crate::{CompiledNode, DataLogic, Error, Result};
use bumpalo::Bump;

/// `throw`. Builds the error object directly from the argument's arena form.
#[inline]
pub(crate) fn evaluate_throw_arena<'a>(
    args: &'a [CompiledNode],
    actx: &mut DataContextStack<'a>,
    engine: &DataLogic,
    arena: &'a Bump,
) -> Result<&'a DataValue<'a>> {
    let owned: OwnedDataValue = if args.is_empty() {
        OwnedDataValue::Null
    } else if let CompiledNode::Value { value, .. } = &args[0] {
        // Literal fast path — skip arena dispatch.
        value.clone()
    } else {
        let av = engine.evaluate_node(&args[0], actx, arena)?;
        av.to_owned()
    };

    let owned = match owned {
        OwnedDataValue::Object(_) => owned,
        OwnedDataValue::String(s) => OwnedDataValue::Object(vec![(
            "type".to_string(),
            OwnedDataValue::String(s),
        )]),
        other => OwnedDataValue::Object(vec![(
            "type".to_string(),
            OwnedDataValue::String(format!("{:?}", other)),
        )]),
    };

    Err(Error::Thrown(owned))
}
