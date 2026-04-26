//! String formatting + truthiness on `ArenaValue`. These produce arena-
//! resident strings (when allocation is needed) so chained string-building
//! operators (`cat`, `substr`, …) avoid heap traffic.

use bumpalo::Bump;

use super::ArenaValue;
use super::conversion::arena_to_value;

/// Render an `ArenaValue` as a `&'a str` allocated in the arena (or borrowed
/// when already a string). Mirrors `helpers::to_string_cow` but produces
/// arena-resident strings so string-building operators (cat, substr) can
/// chain without heap traffic.
pub(crate) fn to_string_arena<'a>(v: &ArenaValue<'a>, arena: &'a Bump) -> &'a str {
    match v {
        ArenaValue::String(s) => s,
        ArenaValue::Null => "",
        ArenaValue::Bool(true) => "true",
        ArenaValue::Bool(false) => "false",
        ArenaValue::Number(n) => arena.alloc_str(&n.to_string()),
        // Composite types: serialize as JSON. Rare path; cost acceptable.
        other => arena.alloc_str(&arena_to_value(other).to_string()),
    }
}

/// Config-aware truthiness for `ArenaValue`. Mirrors `helpers::is_truthy`.
pub(crate) fn is_truthy_arena(v: &ArenaValue<'_>, engine: &crate::DataLogic) -> bool {
    use crate::config::TruthyEvaluator;
    match &engine.config().truthy_evaluator {
        TruthyEvaluator::JavaScript | TruthyEvaluator::Python => v.is_truthy_default(),
        TruthyEvaluator::StrictBoolean => match v {
            ArenaValue::Null => false,
            ArenaValue::Bool(b) => *b,
            _ => true,
        },
        TruthyEvaluator::Custom(f) => f(&arena_to_value(v)),
    }
}
