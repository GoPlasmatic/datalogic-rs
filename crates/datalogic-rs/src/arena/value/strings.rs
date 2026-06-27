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
        // Render the number straight into an arena buffer, skipping the
        // intermediate heap `String` that `to_string()` + `alloc_str` paid.
        // 24 bytes covers every i64 and typical f64 Display, so the buffer
        // does not re-grow in the common case.
        DataValue::Number(n) => {
            use std::fmt::Write as _;
            let mut buf = bumpalo::collections::String::with_capacity_in(24, arena);
            let _ = write!(&mut buf, "{n}");
            buf.into_bump_str()
        }
        // Composite types: serialize as JSON via `datavalue`'s native
        // `Display` emitter. Rare path of unbounded length, so keep the
        // amortized heap `String` build + single exact-size arena copy.
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
