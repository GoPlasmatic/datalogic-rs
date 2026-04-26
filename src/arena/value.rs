//! `ArenaValue` — internal mirror of `serde_json::Value` for arena allocation.
//!
//! Lifetime `'a` is tied to a `bumpalo::Bump` that lives for the duration of
//! one `evaluate()` call. Converted to/from owned `Value` only at API boundaries
//! (input ingestion, output return, custom operator bridges).

use bumpalo::Bump;
use serde_json::Value;
use std::borrow::Cow;

use crate::value::NumberValue;

#[cfg(feature = "datetime")]
use crate::datetime::{DataDateTime, DataDuration};

/// Arena-allocated mirror of [`serde_json::Value`].
///
/// Lifetime `'a` is tied to a [`bumpalo::Bump`] that lives for the
/// duration of one [`crate::DataLogic::evaluate_ref`] call. Composite
/// variants (`String`, `Array`, `Object`) hold arena-allocated slices
/// instead of `Vec` / `BTreeMap` / heap `String` — that's the key
/// allocation win over the public [`Value`] type.
///
/// Most users implementing [`crate::ArenaOperator`] will read inputs via
/// the accessors (`as_f64`, `as_str`, `as_bool`, `is_null`, …) and
/// construct results via the `from_*` constructors, only reaching into
/// the variants directly for advanced cases.
///
/// ## Variants
///
/// - `Null` / `Bool` — inline.
/// - `Number` — wraps [`NumberValue`], distinguishing `Integer(i64)` from
///   `Float(f64)` natively (vs. the opaque `serde_json::Number`).
/// - `String` — UTF-8 bytes allocated in the arena.
/// - `Array` — slice of `ArenaValue` allocated in the arena.
/// - `Object` — slice of `(key, value)` pairs allocated in the arena.
/// - `DateTime` / `Duration` — chrono-backed values, inline. Boundary
///   representation in `serde_json::Value` is `{"datetime": "..."}` /
///   `{"timestamp": "..."}` matching the existing helper contract.
/// - `InputRef` — borrow into the caller's input `&Value` without copying.
///   Used by `var` lookups so input data never gets cloned.
///
/// `Clone` is derived because compiled-time literal precomputation stores a
/// `Box<ArenaValue<'static>>` inside `CompiledNode::Value`, and the enclosing
/// `CompiledNode` derives `Clone`. The clone is shallow for the common
/// primitive variants (Null/Bool/Number/InputRef are Copy-shaped); slice and
/// string variants clone the reference, not the bytes; only DateTime/Duration
/// pay an actual Clone cost — all rare on the hot path.
#[derive(Debug, Clone)]
pub enum ArenaValue<'a> {
    /// JSON null.
    Null,
    /// JSON boolean.
    Bool(bool),
    /// JSON number with native Integer/Float distinction.
    Number(NumberValue),
    /// JSON string allocated in the arena.
    String(&'a str),
    /// JSON array of arena-allocated values.
    Array(&'a [ArenaValue<'a>]),
    /// JSON object as arena-allocated `(key, value)` pairs.
    Object(&'a [(&'a str, ArenaValue<'a>)]),
    /// Chrono-backed datetime (feature `datetime`).
    #[cfg(feature = "datetime")]
    DateTime(DataDateTime),
    /// Chrono-backed duration (feature `datetime`).
    #[cfg(feature = "datetime")]
    Duration(DataDuration),
    /// Zero-clone borrow into the caller's input `&Value`.
    InputRef(&'a Value),
}

impl<'a> ArenaValue<'a> {
    // ---- Constructors ----

    /// Null literal.
    #[inline]
    pub fn null() -> Self {
        ArenaValue::Null
    }

    /// Boolean literal.
    #[inline]
    pub fn bool(b: bool) -> Self {
        ArenaValue::Bool(b)
    }

    /// Numeric literal from i64 (no allocation).
    #[inline]
    pub fn from_i64(i: i64) -> Self {
        ArenaValue::Number(NumberValue::from_i64(i))
    }

    /// Numeric literal from f64. Whole-valued floats within i64 range
    /// collapse to the integer fast path automatically.
    #[inline]
    pub fn from_f64(f: f64) -> Self {
        ArenaValue::Number(NumberValue::from_f64(f))
    }

    /// String literal — allocates the bytes in the arena.
    #[inline]
    pub fn from_str(s: &str, arena: &'a Bump) -> Self {
        ArenaValue::String(arena.alloc_str(s))
    }

    // ---- Accessors ----

    /// Returns true if this value is `Null` or wraps `Value::Null`.
    #[inline]
    pub fn is_null(&self) -> bool {
        matches!(self, ArenaValue::Null | ArenaValue::InputRef(Value::Null))
    }

    /// Extract a boolean if this value is a `Bool` (or wraps one).
    #[inline]
    pub fn as_bool(&self) -> Option<bool> {
        match self {
            ArenaValue::Bool(b) | ArenaValue::InputRef(Value::Bool(b)) => Some(*b),
            _ => None,
        }
    }

    /// Extract an `i64` if this value is integer-valued.
    #[inline]
    pub fn as_i64(&self) -> Option<i64> {
        match self {
            ArenaValue::Number(n) => n.as_i64(),
            ArenaValue::InputRef(Value::Number(n)) => n.as_i64(),
            _ => None,
        }
    }

    /// Extract an `f64` if this value is numeric.
    #[inline]
    pub fn as_f64(&self) -> Option<f64> {
        match self {
            ArenaValue::Number(n) => Some(n.as_f64()),
            ArenaValue::InputRef(Value::Number(n)) => n.as_f64(),
            _ => None,
        }
    }

    /// Extract a string slice if this value is a string.
    #[inline]
    pub fn as_str(&self) -> Option<&str> {
        match self {
            ArenaValue::String(s) => Some(s),
            ArenaValue::InputRef(Value::String(s)) => Some(s.as_str()),
            _ => None,
        }
    }

    /// Truthiness check matching JavaScript-default semantics. For
    /// config-aware truthiness use [`is_truthy_arena`] from the crate root.
    #[inline]
    pub(crate) fn is_truthy_default(&self) -> bool {
        match self {
            ArenaValue::Null => false,
            ArenaValue::Bool(b) => *b,
            ArenaValue::Number(n) => !n.is_zero() && !n.is_nan(),
            ArenaValue::String(s) => !s.is_empty(),
            ArenaValue::Array(items) => !items.is_empty(),
            ArenaValue::Object(pairs) => !pairs.is_empty(),
            #[cfg(feature = "datetime")]
            ArenaValue::DateTime(_) | ArenaValue::Duration(_) => true,
            ArenaValue::InputRef(v) => match v {
                Value::Null => false,
                Value::Bool(b) => *b,
                Value::Number(n) => n.as_f64().is_some_and(|f| f != 0.0 && !f.is_nan()),
                Value::String(s) => !s.is_empty(),
                Value::Array(arr) => !arr.is_empty(),
                Value::Object(obj) => !obj.is_empty(),
            },
        }
    }
}

/// Walk the arena value tree and produce an owned `serde_json::Value`. Called
/// once at the public API boundary on the final result. Allocations here are
/// the same ones the user would have paid for in the non-arena path — we
/// haven't added work, only deferred and consolidated it.
pub(crate) fn arena_to_value(v: &ArenaValue<'_>) -> Value {
    match v {
        ArenaValue::Null => Value::Null,
        ArenaValue::Bool(b) => Value::Bool(*b),
        ArenaValue::Number(n) => match n.to_serde() {
            Some(num) => Value::Number(num),
            None => Value::Null, // NaN/Inf collapse to Null at the JSON boundary.
        },
        ArenaValue::String(s) => Value::String((*s).to_string()),
        ArenaValue::Array(items) => Value::Array(items.iter().map(arena_to_value).collect()),
        ArenaValue::Object(pairs) => {
            let mut map = serde_json::Map::new();
            for (k, v) in *pairs {
                map.insert((*k).to_string(), arena_to_value(v));
            }
            Value::Object(map)
        }
        #[cfg(feature = "datetime")]
        ArenaValue::DateTime(dt) => {
            // Match the boundary contract honored by extract_datetime_value:
            // `{"datetime": "<ISO>"}`. This shape round-trips through the
            // input boundary so a datetime computed in arena mode can be
            // re-fed as input.
            let mut map = serde_json::Map::new();
            map.insert("datetime".to_string(), Value::String(dt.to_iso_string()));
            Value::Object(map)
        }
        #[cfg(feature = "datetime")]
        ArenaValue::Duration(d) => {
            let mut map = serde_json::Map::new();
            map.insert("timestamp".to_string(), Value::String(d.to_string()));
            Value::Object(map)
        }
        ArenaValue::InputRef(v) => (*v).clone(),
    }
}

/// Promote an owned `serde_json::Value` into the arena. Used by the custom
/// operator bridge: a custom op returns owned `Value`, which we wrap in an
/// `ArenaValue` so the rest of the arena pipeline can consume it without
/// special-casing.
///
/// Strings and primitives are copied into the arena. Arrays/objects are
/// recursively converted. Datetime/Duration objects are NOT eagerly parsed
/// here — callers that need temporal semantics extract on demand (matches
/// the existing `extract_datetime_value` contract).
/// Deep-convert a `&Value` into an arena-resident `ArenaValue`. Allocates
/// strings, arrays and objects in `arena`; primitives are inline. Useful for
/// callers that want to pass arena-native input to
/// [`crate::DataLogic::evaluate_in_arena`] without paying the per-call
/// `InputRef` traversal cost.
pub fn value_to_arena<'a>(v: &Value, arena: &'a Bump) -> ArenaValue<'a> {
    match v {
        Value::Null => ArenaValue::Null,
        Value::Bool(b) => ArenaValue::Bool(*b),
        Value::Number(n) => ArenaValue::Number(NumberValue::from_serde(n)),
        Value::String(s) => ArenaValue::String(arena.alloc_str(s)),
        Value::Array(arr) => {
            let items = arena.alloc_slice_fill_iter(arr.iter().map(|x| value_to_arena(x, arena)));
            ArenaValue::Array(items)
        }
        Value::Object(obj) => {
            // Build sorted (key, value) pairs in the arena. JSON objects from
            // serde_json (without `preserve_order`) are already sorted by key
            // because the underlying `Map` is `BTreeMap`. We rely on that here.
            let pairs = arena.alloc_slice_fill_iter(obj.iter().map(|(k, v)| {
                (
                    arena.alloc_str(k.as_str()) as &str,
                    value_to_arena(v, arena),
                )
            }));
            ArenaValue::Object(pairs)
        }
    }
}

// =============================================================================
// Arena-aware helpers — operate on `&ArenaValue<'a>`, deferring to the
// underlying `&Value` only when the variant is `InputRef`.
// =============================================================================

/// Config-aware arena-native f64 coercion. Mirrors
/// `value_helpers::coerce_to_number` exactly — same engine config gates,
/// no `Value` round-trip. For `InputRef` operands the legacy helper is
/// dispatched directly (zero-cost passthrough); for arena-native operands
/// the rules are reproduced inline.
#[inline]
pub(crate) fn coerce_arena_to_number_cfg(
    v: &ArenaValue<'_>,
    engine: &crate::DataLogic,
) -> Option<f64> {
    match v {
        ArenaValue::Number(n) => Some(n.as_f64()),
        ArenaValue::String(s) => {
            if s.is_empty() && engine.config().numeric_coercion.empty_string_to_zero {
                Some(0.0)
            } else {
                s.parse().ok()
            }
        }
        ArenaValue::Bool(b) if engine.config().numeric_coercion.bool_to_number => {
            Some(if *b { 1.0 } else { 0.0 })
        }
        ArenaValue::Null if engine.config().numeric_coercion.null_to_zero => Some(0.0),
        ArenaValue::InputRef(v) => crate::value_helpers::coerce_to_number(v, engine),
        _ => None,
    }
}

/// Config-aware arena-native i64 coercion. Mirrors
/// `value_helpers::try_coerce_to_integer`.
#[inline]
pub(crate) fn try_coerce_arena_to_integer_cfg(
    v: &ArenaValue<'_>,
    engine: &crate::DataLogic,
) -> Option<i64> {
    match v {
        ArenaValue::Number(n) => n.as_i64(),
        ArenaValue::String(s) => {
            if s.is_empty() && engine.config().numeric_coercion.empty_string_to_zero {
                Some(0)
            } else {
                s.parse().ok()
            }
        }
        ArenaValue::Bool(b) if engine.config().numeric_coercion.bool_to_number => {
            Some(if *b { 1 } else { 0 })
        }
        ArenaValue::Null if engine.config().numeric_coercion.null_to_zero => Some(0),
        ArenaValue::InputRef(v) => crate::value_helpers::try_coerce_to_integer(v, engine),
        _ => None,
    }
}

/// Coerce an `ArenaValue` to f64 using the engine's coercion rules. Mirrors
/// `value_helpers::coerce_to_number` but lifts ArenaValue's native variants.
pub(crate) fn coerce_arena_to_number(v: &ArenaValue<'_>) -> Option<f64> {
    match v {
        ArenaValue::Number(n) => Some(n.as_f64()),
        ArenaValue::Bool(true) => Some(1.0),
        ArenaValue::Bool(false) => Some(0.0),
        ArenaValue::Null => Some(0.0),
        ArenaValue::String(s) => {
            let t = s.trim();
            if t.is_empty() {
                Some(0.0)
            } else {
                t.parse().ok()
            }
        }
        ArenaValue::Array(items) => match items.len() {
            0 => Some(0.0),
            1 => coerce_arena_to_number(&items[0]),
            _ => None,
        },
        ArenaValue::Object(_) => None,
        #[cfg(feature = "datetime")]
        ArenaValue::DateTime(_) | ArenaValue::Duration(_) => None,
        ArenaValue::InputRef(v) => coerce_value_to_number_default(v),
    }
}

/// Default JSON Logic numeric coercion against `&serde_json::Value`. Mirrors
/// `value_helpers::coerce_to_number` for the default config (bool→number,
/// null→0, empty string→0). Used when the arena value still holds an
/// `InputRef` to an input-side `Value`.
fn coerce_value_to_number_default(v: &Value) -> Option<f64> {
    match v {
        Value::Number(n) => n.as_f64(),
        Value::Bool(b) => Some(if *b { 1.0 } else { 0.0 }),
        Value::Null => Some(0.0),
        Value::String(s) => {
            let t = s.trim();
            if t.is_empty() {
                Some(0.0)
            } else {
                t.parse().ok()
            }
        }
        Value::Array(arr) => match arr.len() {
            0 => Some(0.0),
            1 => coerce_value_to_number_default(&arr[0]),
            _ => None,
        },
        Value::Object(_) => None,
    }
}

/// Build a fresh `ArenaValue` mirroring the source variant. Used to
/// reborrow into an arena slice (e.g., when constructing an Object from
/// arena-resident field values). Most variants are Copy-shaped; only
/// DateTime/Duration require Clone.
#[inline]
pub(crate) fn reborrow_arena_value<'a>(av: &ArenaValue<'a>) -> ArenaValue<'a> {
    match av {
        ArenaValue::Null => ArenaValue::Null,
        ArenaValue::Bool(b) => ArenaValue::Bool(*b),
        ArenaValue::Number(n) => ArenaValue::Number(*n),
        ArenaValue::String(s) => ArenaValue::String(s),
        ArenaValue::Array(items) => ArenaValue::Array(items),
        ArenaValue::Object(pairs) => ArenaValue::Object(pairs),
        #[cfg(feature = "datetime")]
        ArenaValue::DateTime(dt) => ArenaValue::DateTime(dt.clone()),
        #[cfg(feature = "datetime")]
        ArenaValue::Duration(d) => ArenaValue::Duration(d.clone()),
        ArenaValue::InputRef(v) => ArenaValue::InputRef(v),
    }
}

/// Render an `ArenaValue` as a `Cow<Value>` for use with `&Value`-based
/// helpers (`coerce_to_number`, `try_traverse_segments`, etc.). Borrowed when
/// the source is `InputRef` (zero-cost — most common path for var-lookups);
/// owned otherwise. Owned conversion is cheap for primitives (Number/Bool/Null
/// are inline) but allocates for String/Array/Object/DateTime/Duration.
#[inline]
pub(crate) fn arena_to_value_cow<'a>(v: &'a ArenaValue<'a>) -> Cow<'a, Value> {
    match v {
        ArenaValue::InputRef(vr) => Cow::Borrowed(*vr),
        _ => Cow::Owned(arena_to_value(v)),
    }
}

/// Walk path segments on an `&'a ArenaValue<'a>`. Used by variable-arena
/// non-root lookups when frame data lives in `ArenaContextStack`. Returns
/// `None` if any segment misses or the value isn't traversable.
///
/// Strategy:
/// - `InputRef(v)`: delegate to `try_traverse_segments` (which walks `&Value`)
///   and wrap the leaf as a fresh `InputRef` allocated in the arena.
/// - `Object((&str, ArenaValue))`: linear scan for the key.
/// - `Array([ArenaValue])`: numeric segment.
/// - Anything else: `None`.
pub(crate) fn arena_traverse_segments<'a>(
    av: &'a ArenaValue<'a>,
    segments: &[crate::node::PathSegment],
    arena: &'a Bump,
) -> Option<&'a ArenaValue<'a>> {
    use crate::node::PathSegment;

    if segments.is_empty() {
        return Some(av);
    }

    // InputRef: defer to the `&Value` walker (zero-clone) and wrap the leaf.
    if let ArenaValue::InputRef(v) = av {
        let leaf = crate::operators::variable::try_traverse_segments(v, segments)?;
        return Some(arena.alloc(ArenaValue::InputRef(leaf)));
    }

    let mut cur: &'a ArenaValue<'a> = av;
    for seg in segments {
        match seg {
            PathSegment::Field(key) => match cur {
                ArenaValue::Object(pairs) => {
                    let target: &str = key.as_ref();
                    let mut found: Option<&'a ArenaValue<'a>> = None;
                    for (k, v) in *pairs {
                        if *k == target {
                            // SAFETY: pairs has lifetime 'a, but the entry value
                            // is stored by value. We need an &'a reference to it,
                            // which requires re-allocating in the arena.
                            // Cheap: re-borrow the entry's value reference.
                            let av_ref: &'a ArenaValue<'a> =
                                unsafe { &*(v as *const ArenaValue<'a>) };
                            found = Some(av_ref);
                            break;
                        }
                    }
                    cur = found?;
                }
                ArenaValue::InputRef(Value::Object(obj)) => {
                    let leaf = obj.get(key.as_ref())?;
                    cur = arena.alloc(ArenaValue::InputRef(leaf));
                }
                _ => return None,
            },
            PathSegment::Index(idx) => match cur {
                ArenaValue::Array(items) => {
                    let entry = items.get(*idx)?;
                    let av_ref: &'a ArenaValue<'a> = unsafe { &*(entry as *const ArenaValue<'a>) };
                    cur = av_ref;
                }
                ArenaValue::InputRef(Value::Array(arr)) => {
                    let leaf = arr.get(*idx)?;
                    cur = arena.alloc(ArenaValue::InputRef(leaf));
                }
                _ => return None,
            },
            PathSegment::FieldOrIndex(key, idx) => match cur {
                ArenaValue::Object(pairs) => {
                    let target: &str = key.as_ref();
                    let mut found: Option<&'a ArenaValue<'a>> = None;
                    for (k, v) in *pairs {
                        if *k == target {
                            let av_ref: &'a ArenaValue<'a> =
                                unsafe { &*(v as *const ArenaValue<'a>) };
                            found = Some(av_ref);
                            break;
                        }
                    }
                    cur = found?;
                }
                ArenaValue::Array(items) => {
                    let entry = items.get(*idx)?;
                    let av_ref: &'a ArenaValue<'a> = unsafe { &*(entry as *const ArenaValue<'a>) };
                    cur = av_ref;
                }
                ArenaValue::InputRef(Value::Object(obj)) => {
                    let leaf = obj.get(key.as_ref())?;
                    cur = arena.alloc(ArenaValue::InputRef(leaf));
                }
                ArenaValue::InputRef(Value::Array(arr)) => {
                    let leaf = arr.get(*idx)?;
                    cur = arena.alloc(ArenaValue::InputRef(leaf));
                }
                _ => return None,
            },
        }
    }
    Some(cur)
}

/// Allocation-free path-exists check on `&ArenaValue`. Used by `missing` /
/// `missing_some` where the leaf value isn't consumed.
pub(crate) fn arena_path_exists_str(av: &ArenaValue<'_>, path: &str) -> bool {
    if path.is_empty() {
        return true;
    }
    if let ArenaValue::InputRef(v) = av {
        return crate::value_helpers::access_path_ref(v, path).is_some();
    }

    fn step<'a>(cur: &'a ArenaValue<'a>, seg: &str) -> Option<&'a ArenaValue<'a>> {
        match cur {
            ArenaValue::Object(pairs) => {
                for (k, v) in *pairs {
                    if *k == seg {
                        return Some(v);
                    }
                }
                None
            }
            ArenaValue::Array(items) => {
                let idx = seg.parse::<usize>().ok()?;
                items.get(idx)
            }
            _ => None,
        }
    }

    if !path.contains('.') {
        return step(av, path).is_some();
    }
    let mut cur = av;
    for seg in path.split('.') {
        match step(cur, seg) {
            Some(next) => cur = next,
            None => return false,
        }
    }
    true
}

/// Allocation-free segments-exists check. Companion of [`arena_path_exists_str`]
/// for compile-time-parsed paths.
pub(crate) fn arena_path_exists_segments(
    av: &ArenaValue<'_>,
    segments: &[crate::node::PathSegment],
) -> bool {
    use crate::node::PathSegment;
    if segments.is_empty() {
        return true;
    }
    if let ArenaValue::InputRef(v) = av {
        return crate::operators::variable::try_traverse_segments(v, segments).is_some();
    }

    let mut cur = av;
    for seg in segments {
        let next = match seg {
            PathSegment::Field(key) => match cur {
                ArenaValue::Object(pairs) => {
                    let target: &str = key.as_ref();
                    pairs.iter().find(|(k, _)| *k == target).map(|(_, v)| v)
                }
                _ => None,
            },
            PathSegment::Index(idx) => match cur {
                ArenaValue::Array(items) => items.get(*idx),
                _ => None,
            },
            PathSegment::FieldOrIndex(key, idx) => match cur {
                ArenaValue::Object(pairs) => {
                    let target: &str = key.as_ref();
                    pairs.iter().find(|(k, _)| *k == target).map(|(_, v)| v)
                }
                ArenaValue::Array(items) => items.get(*idx),
                _ => None,
            },
        };
        match next {
            Some(n) => cur = n,
            None => return false,
        }
    }
    true
}

/// Walk a dot-notation `path` on `&'a ArenaValue<'a>`. Mirrors
/// `value_helpers::access_path_ref` for the arena value tree.
///
/// - `InputRef(v)` defers to the existing `&Value` walker and re-wraps the
///   leaf in `InputRef` — zero clone for input-rooted lookups.
/// - `Object(pairs)` does a linear key scan per segment.
/// - `Array(items)` parses the segment as an index.
/// - Anything else (or a missing segment) returns `None`.
pub(crate) fn arena_access_path_str_ref<'a>(
    av: &'a ArenaValue<'a>,
    path: &str,
    arena: &'a Bump,
) -> Option<&'a ArenaValue<'a>> {
    if path.is_empty() {
        return Some(av);
    }

    if let ArenaValue::InputRef(v) = av {
        let leaf = crate::value_helpers::access_path_ref(v, path)?;
        return Some(arena.alloc(ArenaValue::InputRef(leaf)));
    }

    fn step<'a>(cur: &'a ArenaValue<'a>, seg: &str, arena: &'a Bump) -> Option<&'a ArenaValue<'a>> {
        match cur {
            ArenaValue::Object(pairs) => {
                for (k, v) in *pairs {
                    if *k == seg {
                        let av_ref: &'a ArenaValue<'a> = unsafe { &*(v as *const ArenaValue<'a>) };
                        return Some(av_ref);
                    }
                }
                None
            }
            ArenaValue::Array(items) => {
                let idx = seg.parse::<usize>().ok()?;
                let entry = items.get(idx)?;
                let av_ref: &'a ArenaValue<'a> = unsafe { &*(entry as *const ArenaValue<'a>) };
                Some(av_ref)
            }
            ArenaValue::InputRef(Value::Object(obj)) => {
                let leaf = obj.get(seg)?;
                Some(arena.alloc(ArenaValue::InputRef(leaf)))
            }
            ArenaValue::InputRef(Value::Array(arr)) => {
                let idx = seg.parse::<usize>().ok()?;
                let leaf = arr.get(idx)?;
                Some(arena.alloc(ArenaValue::InputRef(leaf)))
            }
            _ => None,
        }
    }

    if !path.contains('.') {
        return step(av, path, arena);
    }

    let mut cur = av;
    for seg in path.split('.') {
        cur = step(cur, seg, arena)?;
    }
    Some(cur)
}

/// Apply a single evaluated path element (string field, numeric index) to an
/// arena value. Mirrors the (deleted) value-mode `apply_path_element_ref` for
/// the multi-arg `val` form where each arg is evaluated separately.
pub(crate) fn arena_apply_path_element<'a>(
    cur: &'a ArenaValue<'a>,
    elem: &ArenaValue<'_>,
    arena: &'a Bump,
) -> Option<&'a ArenaValue<'a>> {
    if let Some(s) = elem.as_str() {
        return arena_access_path_str_ref(cur, s, arena);
    }
    if let Some(i) = elem.as_i64()
        && i >= 0
    {
        let idx = i as usize;
        return match cur {
            ArenaValue::Array(items) => items.get(idx).map(|entry| {
                let av_ref: &'a ArenaValue<'a> = unsafe { &*(entry as *const ArenaValue<'a>) };
                av_ref
            }),
            ArenaValue::InputRef(Value::Array(arr)) => arr
                .get(idx)
                .map(|leaf| &*arena.alloc(ArenaValue::InputRef(leaf))),
            ArenaValue::Object(_) | ArenaValue::InputRef(Value::Object(_)) => {
                arena_access_path_str_ref(cur, &i.to_string(), arena)
            }
            _ => None,
        };
    }
    None
}

/// Render an `ArenaValue` as a `&'a str` allocated in the arena (or borrowed
/// when already a string). Mirrors `helpers::to_string_cow` but produces
/// arena-resident strings so string-building operators (cat, substr) can
/// chain without heap traffic.
pub(crate) fn to_string_arena<'a>(v: &ArenaValue<'a>, arena: &'a Bump) -> &'a str {
    match v {
        ArenaValue::String(s) => s,
        ArenaValue::InputRef(Value::String(s)) => arena.alloc_str(s),
        ArenaValue::Null | ArenaValue::InputRef(Value::Null) => "",
        ArenaValue::Bool(true) | ArenaValue::InputRef(Value::Bool(true)) => "true",
        ArenaValue::Bool(false) | ArenaValue::InputRef(Value::Bool(false)) => "false",
        ArenaValue::Number(n) => arena.alloc_str(&n.to_string()),
        ArenaValue::InputRef(Value::Number(n)) => arena.alloc_str(&n.to_string()),
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
            ArenaValue::InputRef(Value::Null) => false,
            ArenaValue::InputRef(Value::Bool(b)) => *b,
            _ => true,
        },
        TruthyEvaluator::Custom(f) => f(&arena_to_value(v)),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn round_trip(input: Value) {
        let arena = Bump::new();
        let av = value_to_arena(&input, &arena);
        let back = arena_to_value(&av);
        assert_eq!(input, back, "round-trip mismatch");
    }

    #[test]
    fn round_trip_primitives() {
        round_trip(json!(null));
        round_trip(json!(true));
        round_trip(json!(false));
        round_trip(json!(0));
        round_trip(json!(-1));
        round_trip(json!(1.5));
        round_trip(json!(""));
        round_trip(json!("hello"));
        round_trip(json!("with unicode: 日本語 🎉"));
    }

    #[test]
    fn round_trip_arrays() {
        round_trip(json!([]));
        round_trip(json!([1, 2, 3]));
        round_trip(json!([null, true, "x", 1.5, [1, 2]]));
    }

    #[test]
    fn round_trip_objects() {
        round_trip(json!({}));
        round_trip(json!({"a": 1}));
        round_trip(json!({"a": 1, "b": "two", "c": [1, 2, 3]}));
        round_trip(json!({"nested": {"deep": {"value": 42}}}));
    }

    #[test]
    fn round_trip_mixed() {
        round_trip(json!({
            "users": [
                {"id": 1, "name": "Alice", "active": true},
                {"id": 2, "name": "Bob", "active": false}
            ],
            "count": 2,
            "metadata": null
        }));
    }

    #[test]
    fn input_ref_round_trip() {
        let original = json!({"a": [1, 2, 3], "b": "x"});
        let av = ArenaValue::InputRef(&original);
        let back = arena_to_value(&av);
        assert_eq!(original, back);
    }

    #[test]
    fn truthiness_matches_default() {
        let arena = Bump::new();
        assert!(!value_to_arena(&json!(null), &arena).is_truthy_default());
        assert!(!value_to_arena(&json!(false), &arena).is_truthy_default());
        assert!(value_to_arena(&json!(true), &arena).is_truthy_default());
        assert!(!value_to_arena(&json!(0), &arena).is_truthy_default());
        assert!(value_to_arena(&json!(1), &arena).is_truthy_default());
        assert!(!value_to_arena(&json!(""), &arena).is_truthy_default());
        assert!(value_to_arena(&json!("x"), &arena).is_truthy_default());
        assert!(!value_to_arena(&json!([]), &arena).is_truthy_default());
        assert!(value_to_arena(&json!([1]), &arena).is_truthy_default());
        assert!(!value_to_arena(&json!({}), &arena).is_truthy_default());
        assert!(value_to_arena(&json!({"a": 1}), &arena).is_truthy_default());
    }

    #[test]
    fn number_int_path_preserved_through_arena() {
        // Round-tripping `42` must keep it an Integer in NumberValue, and
        // serde_json::Number::from(42) must round-trip back as integer-shaped.
        let v = json!(42);
        let arena = Bump::new();
        let av = value_to_arena(&v, &arena);
        if let ArenaValue::Number(n) = &av {
            assert!(n.is_integer(), "42 should be Integer, got {:?}", n);
            assert_eq!(n.as_i64(), Some(42));
        } else {
            panic!("expected Number variant, got {:?}", av);
        }
        let back = arena_to_value(&av);
        assert_eq!(v, back);
    }

    #[test]
    #[cfg(feature = "datetime")]
    fn datetime_arena_round_trip_via_object() {
        // Boundary: Value::Object {"datetime": "..."} ↔ ArenaValue::DateTime
        // via explicit construction (we don't auto-parse on value_to_arena).
        let dt = DataDateTime::parse("2024-01-15T10:30:00Z").expect("parse");
        let av = ArenaValue::DateTime(dt.clone());
        let back = arena_to_value(&av);
        // back should be {"datetime": "2024-01-15T10:30:00Z"}
        let map = back.as_object().expect("object");
        assert!(map.contains_key("datetime"));
        // And re-parse from the boundary form gives the same DateTime back.
        let s = map
            .get("datetime")
            .and_then(|v| v.as_str())
            .expect("string");
        let dt2 = DataDateTime::parse(s).expect("re-parse");
        assert_eq!(dt, dt2);
    }

    #[test]
    fn coerce_arena_to_number_basics() {
        let arena = Bump::new();
        assert_eq!(
            coerce_arena_to_number(&value_to_arena(&json!(42), &arena)),
            Some(42.0)
        );
        assert_eq!(
            coerce_arena_to_number(&value_to_arena(&json!(true), &arena)),
            Some(1.0)
        );
        assert_eq!(
            coerce_arena_to_number(&value_to_arena(&json!(false), &arena)),
            Some(0.0)
        );
        assert_eq!(
            coerce_arena_to_number(&value_to_arena(&json!(null), &arena)),
            Some(0.0)
        );
        // Numeric string parses; "3.14" round-trips through `coerce_arena_to_number`.
        let parsed: f64 = "3.14".parse().unwrap();
        assert_eq!(
            coerce_arena_to_number(&value_to_arena(&json!("3.14"), &arena)),
            Some(parsed)
        );
        assert_eq!(
            coerce_arena_to_number(&value_to_arena(&json!(""), &arena)),
            Some(0.0)
        );
        assert_eq!(
            coerce_arena_to_number(&value_to_arena(&json!([5]), &arena)),
            Some(5.0)
        );
        assert_eq!(
            coerce_arena_to_number(&value_to_arena(&json!([1, 2]), &arena)),
            None
        );
    }
}
