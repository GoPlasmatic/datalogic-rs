//! Preallocated singleton `DataValue`s.
//!
//! Returning these from operators avoids a per-call `arena.alloc(...)` for
//! every comparison / truthiness branch, the `type` op's eight return
//! strings, and small non-negative integer results (length, index, …).
//!
//! ## Soundness
//!
//! The values are `'static` and contain no arena-borrowed data, so a
//! `&'static DataValue<'static>` is safely castable to `&'a DataValue<'a>`
//! for any caller lifetime `'a`. The `'a` parameter of `DataValue` is
//! covariant, so the lifetime can be shortened freely.

use crate::arena::value::DataValue;
use crate::value::NumberValue;

static SINGLETON_NULL: DataValue<'static> = DataValue::Null;
static SINGLETON_TRUE: DataValue<'static> = DataValue::Bool(true);
static SINGLETON_FALSE: DataValue<'static> = DataValue::Bool(false);
static SINGLETON_EMPTY_STRING: DataValue<'static> = DataValue::String("");
static SINGLETON_EMPTY_ARRAY: DataValue<'static> = DataValue::Array(&[]);
static SINGLETON_EMPTY_OBJECT: DataValue<'static> = DataValue::Object(&[]);

// Type-operator return values — eight fixed strings, returned by every
// `{"type": ...}` dispatch. Static singletons avoid per-call arena writes
// and keep the `type` op allocation-free.
#[cfg(feature = "ext-control")]
static SINGLETON_TYPE_NULL: DataValue<'static> = DataValue::String("null");
#[cfg(feature = "ext-control")]
static SINGLETON_TYPE_BOOL: DataValue<'static> = DataValue::String("boolean");
#[cfg(feature = "ext-control")]
static SINGLETON_TYPE_NUMBER: DataValue<'static> = DataValue::String("number");
#[cfg(feature = "ext-control")]
static SINGLETON_TYPE_STRING: DataValue<'static> = DataValue::String("string");
#[cfg(feature = "ext-control")]
static SINGLETON_TYPE_ARRAY: DataValue<'static> = DataValue::String("array");
#[cfg(feature = "ext-control")]
static SINGLETON_TYPE_OBJECT: DataValue<'static> = DataValue::String("object");
#[cfg(all(feature = "ext-control", feature = "datetime"))]
static SINGLETON_TYPE_DATETIME: DataValue<'static> = DataValue::String("datetime");
#[cfg(all(feature = "ext-control", feature = "datetime"))]
static SINGLETON_TYPE_DURATION: DataValue<'static> = DataValue::String("duration");

/// Borrow the static `Null` singleton at any caller lifetime.
#[inline]
pub(crate) fn singleton_null<'a>() -> &'a DataValue<'a> {
    &SINGLETON_NULL
}

/// Borrow the static `Bool(true)` singleton.
#[inline]
pub(crate) fn singleton_true<'a>() -> &'a DataValue<'a> {
    &SINGLETON_TRUE
}

/// Borrow the static `Bool(false)` singleton.
#[inline]
pub(crate) fn singleton_false<'a>() -> &'a DataValue<'a> {
    &SINGLETON_FALSE
}

/// Borrow the static `Bool(b)` singleton without branching on the caller side.
#[inline]
pub(crate) fn singleton_bool<'a>(b: bool) -> &'a DataValue<'a> {
    if b { &SINGLETON_TRUE } else { &SINGLETON_FALSE }
}

/// Borrow the static empty-string singleton.
#[inline]
pub(crate) fn singleton_empty_string<'a>() -> &'a DataValue<'a> {
    &SINGLETON_EMPTY_STRING
}

/// Borrow the static empty-array singleton.
#[inline]
pub(crate) fn singleton_empty_array<'a>() -> &'a DataValue<'a> {
    &SINGLETON_EMPTY_ARRAY
}

/// Borrow the static empty-object singleton.
#[inline]
pub(crate) fn singleton_empty_object<'a>() -> &'a DataValue<'a> {
    &SINGLETON_EMPTY_OBJECT
}

// Small-integer singletons: covers `0..=SMALL_INT_MAX`. Hits include
// `length`, `var [[N], "index"]` metadata in iteration, integer `reduce`
// results, and any other operator that hands back a small non-negative i64.
// 33 entries × 16 B = 528 B in `.rodata`.
const SMALL_INT_MAX: i64 = 32;

static SINGLETON_SMALL_INTS: [DataValue<'static>; (SMALL_INT_MAX + 1) as usize] = {
    let mut arr = [DataValue::Number(NumberValue::Integer(0)); (SMALL_INT_MAX + 1) as usize];
    let mut i: usize = 0;
    while i < arr.len() {
        arr[i] = DataValue::Number(NumberValue::Integer(i as i64));
        i += 1;
    }
    arr
};

/// Borrow a static `Number(Integer(i))` singleton when `0 <= i <=
/// SMALL_INT_MAX`; returns `None` otherwise so the caller falls back to
/// `arena.alloc(...)`.
#[inline]
pub(crate) fn singleton_small_int<'a>(i: i64) -> Option<&'a DataValue<'a>> {
    if (0..=SMALL_INT_MAX).contains(&i) {
        Some(&SINGLETON_SMALL_INTS[i as usize])
    } else {
        None
    }
}

/// Type-operator return-value singletons. Routed by the literal name the
/// `type` op already produces — no string compare for the caller, just a
/// match on a known set.
#[cfg(feature = "ext-control")]
#[inline]
pub(crate) fn singleton_type_name<'a>(name: &'static str) -> &'a DataValue<'a> {
    match name {
        "null" => &SINGLETON_TYPE_NULL,
        "boolean" => &SINGLETON_TYPE_BOOL,
        "number" => &SINGLETON_TYPE_NUMBER,
        "string" => &SINGLETON_TYPE_STRING,
        "array" => &SINGLETON_TYPE_ARRAY,
        "object" => &SINGLETON_TYPE_OBJECT,
        #[cfg(feature = "datetime")]
        "datetime" => &SINGLETON_TYPE_DATETIME,
        #[cfg(feature = "datetime")]
        "duration" => &SINGLETON_TYPE_DURATION,
        // Unknown name: fall through to a Null singleton. Should be
        // unreachable — `type_op.rs` only ever passes names from the fixed
        // set above — but we want a safe fallback rather than a panic on
        // any future addition that forgets to register here.
        _ => &SINGLETON_NULL,
    }
}
