//! `data_to_str` (arena-resident `&str` rendering) + truthiness on
//! `DataValue`. Both produce arena-resident strings so chained
//! string-building operators (`cat`, `substr`, …) avoid heap traffic.
//!
//! `DataValue → JSON String` is *not* implemented here — `datavalue`'s
//! native `Display` impl emits JSON via its SWAR-driven emitter, and
//! `value.to_string()` is the canonical entry point.

use bumpalo::Bump;

use super::DataValue;

/// Render an `DataValue` as a `&'a str` allocated in the arena (or borrowed
/// when already a string). Mirrors `helpers::to_string_cow` but produces
/// arena-resident strings so string-building operators (cat, substr) can
/// chain without heap traffic.
pub(crate) fn data_to_str<'a>(v: &DataValue<'a>, arena: &'a Bump) -> &'a str {
    match v {
        DataValue::String(s) => s,
        DataValue::Null => "",
        DataValue::Bool(true) => "true",
        DataValue::Bool(false) => "false",
        DataValue::Number(n) => arena.alloc_str(&n.to_string()),
        // Composite types: serialize as JSON via `datavalue`'s native
        // `Display` emitter. Rare path; cost acceptable.
        other => arena.alloc_str(&other.to_string()),
    }
}

/// Config-aware truthiness for `DataValue`. Mirrors `helpers::truthy_arena`.
///
/// `#[inline(always)]` because the function ends up inside the per-iteration
/// general path of every quantifier/filter — outlining was paying a real call
/// per item even though the hot branch is just the JS/Python default.
#[inline(always)]
pub(crate) fn truthy_arena(v: &DataValue<'_>, engine: &crate::Engine) -> bool {
    use crate::config::TruthyEvaluator;
    match &engine.config().truthy_evaluator {
        TruthyEvaluator::JavaScript | TruthyEvaluator::Python => super::truthy_js_arena(v),
        TruthyEvaluator::StrictBoolean => match v {
            DataValue::Null => false,
            DataValue::Bool(b) => *b,
            _ => true,
        },
        TruthyEvaluator::Custom(f) => f(&v.to_owned()),
    }
}
