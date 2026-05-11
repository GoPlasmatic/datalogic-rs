//! Internal `serde_json::Value` ↔ `OwnedDataValue` / `DataValue` helpers.
//!
//! Gated on `feature = "serde_json"`. Used by the input/output traits
//! ([`crate::IntoLogic`], [`crate::EvalInput`], [`crate::FromDataValue`])
//! and by the trace surface, which records steps as
//! `serde_json::Value`.
//!
//! Not part of the public API: the helpers are `pub(crate)` so the
//! conversion logic stays in one place without leaking through the
//! crate root.

use datavalue::OwnedDataValue;
use serde_json::Value;

/// Walk a `serde_json::Value` and produce an [`OwnedDataValue`]. Thin
/// wrapper over `OwnedDataValue::from(&serde_json::Value)`.
#[inline]
pub(crate) fn owned_from_serde(v: &Value) -> OwnedDataValue {
    OwnedDataValue::from(v)
}

/// Inverse of [`owned_from_serde`] — walk an [`OwnedDataValue`] and
/// produce a `serde_json::Value`. Delegates to `datavalue` for non-datetime
/// shapes; wraps DateTime/Duration in datalogic sentinel objects so values
/// produced inside the engine round-trip through the JSON boundary.
pub(crate) fn owned_to_serde(v: &OwnedDataValue) -> Value {
    match v {
        #[cfg(feature = "datetime")]
        OwnedDataValue::DateTime(dt) => datetime_sentinel("datetime", dt.to_iso_string()),
        #[cfg(feature = "datetime")]
        OwnedDataValue::Duration(d) => datetime_sentinel("timestamp", d.to_string()),
        OwnedDataValue::Array(items) => Value::Array(items.iter().map(owned_to_serde).collect()),
        OwnedDataValue::Object(pairs) => Value::Object(
            pairs
                .iter()
                .map(|(k, v)| (k.clone(), owned_to_serde(v)))
                .collect(),
        ),
        other => other.to_serde_value(),
    }
}

/// Wrap a datetime / duration in the datalogic sentinel-object form
/// (`{datetime: <iso>}` or `{timestamp: <iso>}`). Shared by the
/// `OwnedDataValue` and arena `DataValue` serializers so the wrapping
/// stays consistent in one place.
#[cfg(feature = "datetime")]
#[inline]
pub(crate) fn datetime_sentinel(key: &str, payload: String) -> Value {
    let mut map = serde_json::Map::new();
    map.insert(key.to_string(), Value::String(payload));
    Value::Object(map)
}
