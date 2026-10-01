//! Boundary helpers around the arena value type.
//!
//! The `serde_json::Value` ↔ `DataValue` bridges live behind the
//! `serde_json` feature — used by the public `eval*` paths to accept and
//! return `serde_json::Value` inputs/outputs. The conversion logic
//! itself is delegated to `datavalue`'s built-in serde_json bridge;
//! only DateTime / Duration get wrapped here in their datalogic
//! sentinel form (`{"datetime": "..."}`, `{"timestamp": "..."}`) which
//! datavalue does not preserve.

#[cfg(feature = "serde_json")]
pub(crate) use serde_impl::data_to_value;
#[cfg(feature = "serde_json")]
pub(crate) use serde_impl::value_to_data;

#[cfg(feature = "serde_json")]
mod serde_impl {
    use super::super::DataValue;
    use bumpalo::Bump;
    use serde_json::Value;

    /// Walk the arena value tree and produce an owned `serde_json::Value`.
    /// Delegates to `datavalue::DataValue::to_serde_value` for non-datetime
    /// shapes; wraps DateTime/Duration in the datalogic sentinel form so
    /// values produced inside the engine round-trip back through the input
    /// boundary.
    pub(crate) fn data_to_value(v: &DataValue<'_>) -> Value {
        match v {
            #[cfg(feature = "datetime")]
            DataValue::DateTime(dt) => {
                crate::serde_bridge::datetime_sentinel("datetime", dt.to_iso_string())
            }
            #[cfg(feature = "datetime")]
            DataValue::Duration(d) => {
                crate::serde_bridge::datetime_sentinel("timestamp", d.to_string())
            }
            // Composite arms recurse through datavalue, but a DateTime nested
            // inside an Array/Object would lose its sentinel form. Walk
            // composites manually so the recursion routes datetimes back
            // through the sentinel-aware arms above.
            DataValue::Array(items) => Value::Array(items.iter().map(data_to_value).collect()),
            DataValue::Object(pairs) => {
                let mut map = serde_json::Map::with_capacity(pairs.len());
                for (k, v) in *pairs {
                    map.insert((*k).to_string(), data_to_value(v));
                }
                Value::Object(map)
            }
            // Scalars: delegate to datavalue.
            other => other.to_serde_value(),
        }
    }

    /// View a `&Value` as an arena `DataValue`: strings and object keys
    /// are borrowed from `v`, and only the array and object spines are
    /// built in `arena` (`datavalue`'s `from_serde_value_in` copies every
    /// string). Numbers convert exactly as that function converts them.
    pub(crate) fn value_to_data<'a>(v: &'a Value, arena: &'a Bump) -> DataValue<'a> {
        match v {
            Value::Null => DataValue::Null,
            Value::Bool(b) => DataValue::Bool(*b),
            Value::Number(n) => DataValue::Number(number(n)),
            Value::String(s) => DataValue::String(s.as_str()),
            Value::Array(items) => DataValue::Array(
                arena.alloc_slice_fill_iter(items.iter().map(|item| value_to_data(item, arena))),
            ),
            Value::Object(map) => DataValue::Object(
                arena.alloc_slice_fill_iter(
                    map.iter()
                        .map(|(k, item)| (k.as_str(), value_to_data(item, arena))),
                ),
            ),
        }
    }

    /// `serde_json::Number` to `NumberValue`, the same mapping as
    /// `datavalue`'s serde bridge: i64 when it fits, then u64 (whose
    /// out-of-i64 values take `from_u64`'s f64 fallback), then f64.
    fn number(n: &serde_json::Number) -> datavalue::NumberValue {
        use datavalue::NumberValue;
        if let Some(i) = n.as_i64() {
            NumberValue::Integer(i)
        } else if let Some(u) = n.as_u64() {
            NumberValue::from_u64(u)
        } else {
            NumberValue::Float(n.as_f64().unwrap_or(0.0))
        }
    }
}
