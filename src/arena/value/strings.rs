//! String formatting + truthiness on `DataValue`. These produce arena-
//! resident strings (when allocation is needed) so chained string-building
//! operators (`cat`, `substr`, …) avoid heap traffic.

use bumpalo::Bump;
use std::fmt::Write;

use super::DataValue;

/// Render an `DataValue` as a `&'a str` allocated in the arena (or borrowed
/// when already a string). Mirrors `helpers::to_string_cow` but produces
/// arena-resident strings so string-building operators (cat, substr) can
/// chain without heap traffic.
pub(crate) fn to_string_arena<'a>(v: &DataValue<'a>, arena: &'a Bump) -> &'a str {
    match v {
        DataValue::String(s) => s,
        DataValue::Null => "",
        DataValue::Bool(true) => "true",
        DataValue::Bool(false) => "false",
        DataValue::Number(n) => arena.alloc_str(&n.to_string()),
        // Composite types: serialize as JSON. Rare path; cost acceptable.
        other => {
            let mut buf = String::new();
            write_data_json(&mut buf, other);
            arena.alloc_str(&buf)
        }
    }
}

/// Render a [`DataValue`] as a JSON `String`. Public crate-internal so the
/// engine's v5 boundary helper can serialize results without pulling in
/// `serde_json`.
#[inline]
pub(crate) fn data_to_json_string(v: &DataValue<'_>) -> String {
    let mut buf = String::new();
    write_data_json(&mut buf, v);
    buf
}

/// Tiny manual JSON serializer for [`DataValue`]. Avoids pulling in
/// `serde_json` for the rare arena-string-rendering path. Format matches
/// `serde_json::to_string` byte-for-byte for the variants we emit.
fn write_data_json(buf: &mut String, v: &DataValue<'_>) {
    match v {
        DataValue::Null => buf.push_str("null"),
        DataValue::Bool(true) => buf.push_str("true"),
        DataValue::Bool(false) => buf.push_str("false"),
        DataValue::Number(n) => {
            // Number's Display already matches the JSON shape (NaN/Inf → null).
            let _ = write!(buf, "{}", n);
        }
        DataValue::String(s) => write_json_string(buf, s),
        DataValue::Array(items) => {
            buf.push('[');
            for (i, it) in items.iter().enumerate() {
                if i > 0 {
                    buf.push(',');
                }
                write_data_json(buf, it);
            }
            buf.push(']');
        }
        DataValue::Object(pairs) => {
            buf.push('{');
            for (i, (k, val)) in pairs.iter().enumerate() {
                if i > 0 {
                    buf.push(',');
                }
                write_json_string(buf, k);
                buf.push(':');
                write_data_json(buf, val);
            }
            buf.push('}');
        }
        #[cfg(feature = "datetime")]
        DataValue::DateTime(dt) => {
            buf.push_str("{\"datetime\":");
            write_json_string(buf, &dt.to_iso_string());
            buf.push('}');
        }
        #[cfg(feature = "datetime")]
        DataValue::Duration(d) => {
            buf.push_str("{\"timestamp\":");
            write_json_string(buf, &d.to_string());
            buf.push('}');
        }
    }
}

fn write_json_string(buf: &mut String, s: &str) {
    buf.push('"');
    for c in s.chars() {
        match c {
            '"' => buf.push_str("\\\""),
            '\\' => buf.push_str("\\\\"),
            '\n' => buf.push_str("\\n"),
            '\r' => buf.push_str("\\r"),
            '\t' => buf.push_str("\\t"),
            '\x08' => buf.push_str("\\b"),
            '\x0c' => buf.push_str("\\f"),
            c if (c as u32) < 0x20 => {
                let _ = write!(buf, "\\u{:04x}", c as u32);
            }
            c => buf.push(c),
        }
    }
    buf.push('"');
}

/// Config-aware truthiness for `DataValue`. Mirrors `helpers::is_truthy`.
///
/// `#[inline(always)]` because the function ends up inside the per-iteration
/// general path of every quantifier/filter — outlining was paying a real call
/// per item even though the hot branch is just the JS/Python default.
#[inline(always)]
pub(crate) fn is_truthy_arena(v: &DataValue<'_>, engine: &crate::DataLogic) -> bool {
    use crate::config::TruthyEvaluator;
    match &engine.config().truthy_evaluator {
        TruthyEvaluator::JavaScript | TruthyEvaluator::Python => super::is_truthy_default(v),
        TruthyEvaluator::StrictBoolean => match v {
            DataValue::Null => false,
            DataValue::Bool(b) => *b,
            _ => true,
        },
        #[cfg(feature = "compat")]
        TruthyEvaluator::Custom(f) => f(&super::conversion::arena_to_value(v)),
    }
}
