//! Re-exports of the numeric type used by the arena evaluation path.
//!
//! `NumberValue` lives in the `datavalue` crate. The two crates ship the
//! same shape (`Integer(i64)` / `Float(f64)`) so this is a transparent
//! re-export. Compat shims for `serde_json::Number` interop live alongside
//! the rest of the compat surface.

pub use datavalue::NumberValue;

/// Construct a [`NumberValue`] from a `serde_json::Number`. Used by the
/// `compat` layer when bridging old `serde_json::Value` callers; not part
/// of the v5 surface.
#[cfg(feature = "compat")]
#[inline]
pub(crate) fn number_from_serde(n: &serde_json::Number) -> NumberValue {
    if let Some(i) = n.as_i64() {
        NumberValue::Integer(i)
    } else if let Some(u) = n.as_u64() {
        if u <= i64::MAX as u64 {
            NumberValue::Integer(u as i64)
        } else {
            NumberValue::Float(u as f64)
        }
    } else {
        NumberValue::Float(n.as_f64().unwrap_or(0.0))
    }
}

/// Convert a [`NumberValue`] back into a `serde_json::Number` for the
/// compat boundary. NaN/Inf return `None` since `serde_json::Number`
/// rejects them.
#[cfg(feature = "compat")]
#[inline]
pub(crate) fn number_to_serde(n: NumberValue) -> Option<serde_json::Number> {
    match n {
        NumberValue::Integer(i) => Some(serde_json::Number::from(i)),
        NumberValue::Float(f) => serde_json::Number::from_f64(f),
    }
}

/// Walk a `serde_json::Value` and produce an [`OwnedDataValue`]. Used by the
/// compat layer (and by intermediate engine paths still working in
/// serde_json terms) to lift a parsed JSON tree into the v5 value type.
#[cfg(feature = "compat")]
pub(crate) fn owned_from_serde(v: &serde_json::Value) -> datavalue::OwnedDataValue {
    use datavalue::OwnedDataValue;
    match v {
        serde_json::Value::Null => OwnedDataValue::Null,
        serde_json::Value::Bool(b) => OwnedDataValue::Bool(*b),
        serde_json::Value::Number(n) => OwnedDataValue::Number(number_from_serde(n)),
        serde_json::Value::String(s) => OwnedDataValue::String(s.clone()),
        serde_json::Value::Array(arr) => {
            OwnedDataValue::Array(arr.iter().map(owned_from_serde).collect())
        }
        serde_json::Value::Object(obj) => OwnedDataValue::Object(
            obj.iter()
                .map(|(k, v)| (k.clone(), owned_from_serde(v)))
                .collect(),
        ),
    }
}

/// Inverse of [`owned_from_serde`] — walk an [`OwnedDataValue`] and produce
/// a `serde_json::Value`. Compat-layer boundary helper.
#[cfg(feature = "compat")]
pub(crate) fn owned_to_serde(v: &datavalue::OwnedDataValue) -> serde_json::Value {
    use datavalue::OwnedDataValue;
    match v {
        OwnedDataValue::Null => serde_json::Value::Null,
        OwnedDataValue::Bool(b) => serde_json::Value::Bool(*b),
        OwnedDataValue::Number(n) => match number_to_serde(*n) {
            Some(num) => serde_json::Value::Number(num),
            None => serde_json::Value::Null,
        },
        OwnedDataValue::String(s) => serde_json::Value::String(s.clone()),
        OwnedDataValue::Array(items) => {
            serde_json::Value::Array(items.iter().map(owned_to_serde).collect())
        }
        OwnedDataValue::Object(pairs) => serde_json::Value::Object(
            pairs.iter().map(|(k, v)| (k.clone(), owned_to_serde(v))).collect(),
        ),
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
    }
}
