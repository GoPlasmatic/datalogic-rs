//! Thread-local Bump pool — amortizes the per-call `Bump::with_capacity` cost
//! that profiling identified at ~6% of compatible.json CPU.
//!
//! ## Design (matches ARENA_RFC §6.2)
//!
//! Each OS thread keeps a small bounded pool of recycled `Bump`s.
//! `acquire()` pops one (or creates a new sized arena if the pool is empty);
//! the returned `ArenaGuard` returns the arena to the pool on drop after
//! `Bump::reset()` (O(1) — no individual allocations are freed).
//!
//! ## Why TLS, not `bumpalo-herd`
//!
//! `bumpalo-herd` ties allocator lifetime to scoped threads / rayon, which
//! breaks under Tokio's work-stealing scheduler. Our TLS pool is safe in
//! async contexts because acquire/release happens synchronously within one
//! `evaluate()` call — there's no `.await` between them.
//!
//! ## Safety properties
//!
//! - `DataLogic: Send + Sync` is preserved (no shared engine state added)
//! - Each thread has its own pool — no cross-thread contention
//! - Pool is bounded (4 entries) so memory is capped per thread
//! - The guard borrow-checks: `&Bump` cannot outlive the guard, so an
//!   `DataValue<'a>` cannot accidentally survive into the next pool reuse.

use bumpalo::Bump;
use std::cell::Cell;
use std::mem::ManuallyDrop;

use crate::arena::value::DataValue;
use crate::value::NumberValue;

// =============================================================================
// Preallocated singletons. Returning these from operators avoids a per-call
// `arena.alloc(DataValue::Bool(true))` for every comparison / truthiness
// branch.
//
// Soundness: the values are `'static` and contain no arena-borrowed data,
// so a `&'static DataValue<'static>` is safely castable to `&'a DataValue<'a>`
// for any caller lifetime `'a`. The `'a` parameter of the destination is
// covariant in DataValue, so the lifetime can be shortened freely.
// =============================================================================

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

// Single-slot per-thread arena reuse. Replaced the previous `RefCell<Vec<Bump>>`
// pool because the per-call cost of `RefCell::borrow_mut` + `Vec::pop`/`push`
// showed up in profiling and a single slot covers the common case (one
// in-flight `evaluate()` per thread). Re-entrancy is handled gracefully: a
// nested `evaluate()` finds the slot empty and allocates a fresh `Bump`; on
// nested-eval drop the slot may already be occupied by the outer call's
// returned bump, in which case the nested one is dropped (its chunks
// reclaimed). We trade a small re-entrancy reuse loss for one TLS access +
// `Cell::take` instead of a TLS access + `RefCell::borrow_mut` + `Vec::pop`.

thread_local! {
    static ARENA_SLOT: Cell<Option<Bump>> = const { Cell::new(None) };
}

/// RAII guard that owns a `Bump` for the lifetime of one `evaluate()` call
/// and returns it to the thread-local slot on drop.
///
/// Use `guard.arena()` to get a `&Bump` (whose lifetime is bounded by the
/// guard, so `DataValue<'_>` cannot escape the call).
#[allow(dead_code)] // Test-only utility after v5 funnel landed.
pub(crate) struct ArenaGuard {
    /// `ManuallyDrop` lets `Drop::drop` move the `Bump` back into the slot
    /// without violating `Drop`'s `&mut self` aliasing rules.
    arena: ManuallyDrop<Bump>,
}

#[allow(dead_code)] // Test-only utility after v5 funnel landed.
impl ArenaGuard {
    /// Take the thread's `Bump` from the slot, or allocate a fresh one sized
    /// to `min_capacity` if the slot is empty.
    #[inline]
    pub(crate) fn acquire(min_capacity: usize) -> Self {
        let bump = ARENA_SLOT
            .with(|slot| slot.take())
            .unwrap_or_else(|| Bump::with_capacity(min_capacity));
        Self {
            arena: ManuallyDrop::new(bump),
        }
    }

    /// The bump arena. Lifetime is bounded by `&self`, so allocations made
    /// against this `Bump` cannot outlive the guard.
    #[inline]
    pub(crate) fn arena(&self) -> &Bump {
        &self.arena
    }
}

impl Drop for ArenaGuard {
    #[inline]
    fn drop(&mut self) {
        // SAFETY: `arena` is `ManuallyDrop`-wrapped and we only take it once,
        // here, on Drop. After this point `self.arena` is logically uninit
        // and not accessed again.
        let mut bump = unsafe { ManuallyDrop::take(&mut self.arena) };

        // Skip reset when the arena saw no allocations — `Bump::reset()`
        // walks every chunk, and singleton-only evaluations (`{"==": [1, 1]}`,
        // pure var lookups, etc.) leave the arena untouched.
        if bump.allocated_bytes() != 0 {
            bump.reset();
        }

        // Put back into the per-thread slot. `replace` returns the previous
        // occupant (None in the common single-eval-at-a-time case, Some on
        // re-entrant drop where the inner guard already filled it — that
        // older bump simply drops here, freeing its chunks).
        ARENA_SLOT.with(|slot| {
            let _evicted = slot.replace(Some(bump));
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn drain_slot() {
        ARENA_SLOT.with(|s| {
            let _ = s.take();
        });
    }

    #[test]
    fn acquire_release_reuses_arena() {
        drain_slot();

        // First acquire: slot empty, fresh Bump.
        let g1 = ArenaGuard::acquire(4096);
        let _ = g1.arena().alloc_str("hello");
        drop(g1);

        // Slot should now hold the released arena.
        let occupied = ARENA_SLOT.with(|s| {
            let b = s.take();
            let occupied = b.is_some();
            s.set(b);
            occupied
        });
        assert!(occupied, "released arena should be in the slot");

        // Second acquire: should take the previous one (slot becomes empty).
        let g2 = ArenaGuard::acquire(4096);
        let still_empty = ARENA_SLOT.with(|s| {
            let b = s.take();
            let empty = b.is_none();
            s.set(b);
            empty
        });
        assert!(still_empty, "acquire took from slot");
        drop(g2);
    }

    #[test]
    fn nested_acquire_drops_inner() {
        // Re-entrant case: outer acquires, then inner acquires (slot is empty
        // since outer took it), then inner drops (puts its bump in slot),
        // then outer drops — the slot already holds inner's bump, so outer's
        // bump replaces it and inner's bump is freed.
        drain_slot();

        let outer = ArenaGuard::acquire(4096);
        let inner = ArenaGuard::acquire(4096);
        let _ = inner.arena().alloc_str("inner");
        drop(inner);
        let _ = outer.arena().alloc_str("outer");
        drop(outer);

        // Slot ends up holding exactly one bump (outer's).
        let count = ARENA_SLOT.with(|s| {
            let b = s.take();
            let count = if b.is_some() { 1 } else { 0 };
            s.set(b);
            count
        });
        assert_eq!(count, 1);
    }

    #[test]
    fn reset_makes_arena_reusable() {
        drain_slot();

        let g1 = ArenaGuard::acquire(4096);
        let s1 = g1.arena().alloc_str("first");
        assert_eq!(s1, "first");
        drop(g1);

        let g2 = ArenaGuard::acquire(4096);
        let s2 = g2.arena().alloc_str("second");
        assert_eq!(s2, "second");
        drop(g2);
    }
}
