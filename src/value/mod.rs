//! Re-exports of the numeric type used by the arena evaluation path.
//!
//! `NumberValue` lives in the `datavalue` crate. The two crates ship the
//! same shape (`Integer(i64)` / `Float(f64)`) so this is a transparent
//! re-export. The serde_json bridges are also datavalue-native (under its
//! `serde_json` feature, enabled by `datalogic-rs/compat`); the helpers
//! here just thread the datalogic DateTime/Duration sentinel format
//! (`{"datetime": "..."}`, `{"timestamp": "..."}`) which datavalue does
//! not preserve.

pub use datavalue::NumberValue;

/// Walk a `serde_json::Value` and produce an [`OwnedDataValue`]. Thin
/// wrapper over `OwnedDataValue::from(&serde_json::Value)`; provided as
/// the named entry point for the v4 compat surface.
///
/// Available under `feature = "compat"` — non-compat builds have no serde_json
/// dependency.
#[cfg(feature = "compat")]
#[inline]
pub fn owned_from_serde(v: &serde_json::Value) -> datavalue::OwnedDataValue {
    datavalue::OwnedDataValue::from(v)
}

/// Inverse of [`owned_from_serde`] — walk an [`OwnedDataValue`] and produce
/// a `serde_json::Value`. Delegates to `datavalue` for non-datetime shapes;
/// wraps DateTime/Duration in datalogic sentinel objects so values produced
/// inside the engine round-trip through the v4 input boundary.
#[cfg(feature = "compat")]
pub fn owned_to_serde(v: &datavalue::OwnedDataValue) -> serde_json::Value {
    use datavalue::OwnedDataValue;
    match v {
        #[cfg(feature = "datetime")]
        OwnedDataValue::DateTime(dt) => {
            let mut map = serde_json::Map::new();
            map.insert(
                "datetime".to_string(),
                serde_json::Value::String(dt.to_iso_string()),
            );
            serde_json::Value::Object(map)
        }
        #[cfg(feature = "datetime")]
        OwnedDataValue::Duration(d) => {
            let mut map = serde_json::Map::new();
            map.insert(
                "timestamp".to_string(),
                serde_json::Value::String(d.to_string()),
            );
            serde_json::Value::Object(map)
        }
        // Composite arms recurse manually so nested DateTime/Duration values
        // route back through the sentinel-wrapping arms above.
        OwnedDataValue::Array(items) => {
            serde_json::Value::Array(items.iter().map(owned_to_serde).collect())
        }
        OwnedDataValue::Object(pairs) => serde_json::Value::Object(
            pairs
                .iter()
                .map(|(k, v)| (k.clone(), owned_to_serde(v)))
                .collect(),
        ),
        // Scalars: delegate to datavalue.
        other => other.to_serde_value(),
    }
}
