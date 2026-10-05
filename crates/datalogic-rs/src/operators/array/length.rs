//! `length` — string char count or array length.

use crate::Result;
use crate::arena::DataValue;
use crate::operators::eager::Cx;

/// `length(value)`: a string's char count or an array's item count.
/// Composed calls (`length(filter(...))`) read the arena-resident
/// intermediate's slice length directly: zero conversion cost.
#[inline]
pub(crate) fn length<'a>(cx: &mut Cx<'_, 'a>, arg: &'a DataValue<'a>) -> Result<i64> {
    match arg {
        // Counting chars walks the string.
        DataValue::String(s) => {
            cx.charge_bytes(s.len())?;
            Ok(s.chars().count() as i64)
        }
        DataValue::Array(items) => Ok(items.len() as i64),
        _ => Err(crate::Error::invalid_args()),
    }
}
