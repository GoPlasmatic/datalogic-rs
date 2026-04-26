//! Thread-local Bump pool — amortizes the per-call `Bump::with_capacity` cost
//! seen at ~6% of compatible.json CPU in Phase 5 profiling.
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
//!   `ArenaValue<'a>` cannot accidentally survive into the next pool reuse.

use bumpalo::Bump;
use std::cell::Cell;
use std::mem::ManuallyDrop;

use crate::arena::value::ArenaValue;

// =============================================================================
// Preallocated singletons. Mirrors v3's `null_value` / `true_value` /
// `false_value` / `empty_string_value` / `empty_array_value` (v3.0.6
// `src/arena/bump.rs:106-120`). Returning these from operators avoids a
// per-call `arena.alloc(ArenaValue::Bool(true))` for every comparison /
// truthiness branch.
//
// Soundness: the values are `'static` and contain no arena-borrowed data,
// so a `&'static ArenaValue<'static>` is safely castable to `&'a ArenaValue<'a>`
// for any caller lifetime `'a`. The `'a` parameter of the destination is
// covariant in ArenaValue, so the lifetime can be shortened freely.
// =============================================================================

static SINGLETON_NULL: ArenaValue<'static> = ArenaValue::Null;
static SINGLETON_TRUE: ArenaValue<'static> = ArenaValue::Bool(true);
static SINGLETON_FALSE: ArenaValue<'static> = ArenaValue::Bool(false);
static SINGLETON_EMPTY_STRING: ArenaValue<'static> = ArenaValue::String("");
static SINGLETON_EMPTY_ARRAY: ArenaValue<'static> = ArenaValue::Array(&[]);

/// Borrow the static `Null` singleton at any caller lifetime.
#[inline]
#[allow(dead_code)]
pub(crate) fn singleton_null<'a>() -> &'a ArenaValue<'a> {
    &SINGLETON_NULL
}

/// Borrow the static `Bool(true)` singleton.
#[inline]
#[allow(dead_code)]
pub(crate) fn singleton_true<'a>() -> &'a ArenaValue<'a> {
    &SINGLETON_TRUE
}

/// Borrow the static `Bool(false)` singleton.
#[inline]
#[allow(dead_code)]
pub(crate) fn singleton_false<'a>() -> &'a ArenaValue<'a> {
    &SINGLETON_FALSE
}

/// Borrow the static `Bool(b)` singleton without branching on the caller side.
#[inline]
#[allow(dead_code)]
pub(crate) fn singleton_bool<'a>(b: bool) -> &'a ArenaValue<'a> {
    if b { &SINGLETON_TRUE } else { &SINGLETON_FALSE }
}

/// Borrow the static empty-string singleton.
#[inline]
#[allow(dead_code)]
pub(crate) fn singleton_empty_string<'a>() -> &'a ArenaValue<'a> {
    &SINGLETON_EMPTY_STRING
}

/// Borrow the static empty-array singleton.
#[inline]
#[allow(dead_code)]
pub(crate) fn singleton_empty_array<'a>() -> &'a ArenaValue<'a> {
    &SINGLETON_EMPTY_ARRAY
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
/// guard, so `ArenaValue<'_>` cannot escape the call).
pub(crate) struct ArenaGuard {
    /// `ManuallyDrop` lets `Drop::drop` move the `Bump` back into the slot
    /// without violating `Drop`'s `&mut self` aliasing rules.
    arena: ManuallyDrop<Bump>,
}

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
