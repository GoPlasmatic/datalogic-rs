//! Boundary helpers around the arena value type.
//!
//! The `serde_json::Value` ↔ `DataValue` bridges live behind the `compat`
//! feature — they're only needed for the deprecated v4 API surface in
//! `crate::compat`. The conversion logic itself is delegated to
//! `datavalue`'s built-in serde_json bridge; only DateTime / Duration get
//! wrapped here in their datalogic sentinel form (`{"datetime": "..."}`,
//! `{"timestamp": "..."}`) which datavalue does not preserve.

#[cfg(feature = "compat")]
pub use compat_impl::data_to_value;
#[cfg(feature = "compat")]
pub use compat_impl::value_to_data;

#[cfg(feature = "compat")]
mod compat_impl {
    use super::super::DataValue;
    use bumpalo::Bump;
    use serde_json::Value;

    /// Walk the arena value tree and produce an owned `serde_json::Value`.
    /// Delegates to `datavalue::DataValue::to_serde_value` for non-datetime
    /// shapes; wraps DateTime/Duration in the datalogic sentinel form so
    /// values produced inside the engine round-trip back through the input
    /// boundary.
    pub fn data_to_value(v: &DataValue<'_>) -> Value {
        match v {
            #[cfg(feature = "datetime")]
            DataValue::DateTime(dt) => {
                crate::compat::datetime_sentinel("datetime", dt.to_iso_string())
            }
            #[cfg(feature = "datetime")]
            DataValue::Duration(d) => crate::compat::datetime_sentinel("timestamp", d.to_string()),
            // Composite arms recurse through datavalue, but a DateTime nested
            // inside an Array/Object would lose its sentinel form. Walk
            // composites manually so the recursion routes datetimes back
            // through the sentinel-aware arms above.
            DataValue::Array(items) => Value::Array(items.iter().map(data_to_value).collect()),
            DataValue::Object(pairs) => {
                let mut map = serde_json::Map::new();
                for (k, v) in *pairs {
                    map.insert((*k).to_string(), data_to_value(v));
                }
                Value::Object(map)
            }
            // Scalars: delegate to datavalue.
            other => other.to_serde_value(),
        }
    }

    /// Deep-convert a `&Value` into an arena-resident `DataValue`. Thin
    /// wrapper over `datavalue::DataValue::from_serde_value_in`; provided
    /// as a v4 compat surface entry point.
    pub fn value_to_data<'a>(v: &Value, arena: &'a Bump) -> DataValue<'a> {
        DataValue::from_serde_value_in(v, arena)
    }
}
