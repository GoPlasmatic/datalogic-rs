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
use std::cell::RefCell;
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

/// How many `Bump`s we keep per thread. Tradeoff: larger = better warm-up
/// for nested/concurrent eval calls on the same thread; smaller = less idle
/// memory. Four covers re-entrant evaluator patterns without bloat.
const POOL_MAX: usize = 4;

thread_local! {
    static ARENA_POOL: RefCell<Vec<Bump>> = const { RefCell::new(Vec::new()) };
}

/// RAII guard that owns a `Bump` for the lifetime of one `evaluate()` call
/// and returns it to the thread-local pool on drop.
///
/// Use `guard.arena()` to get a `&Bump` (whose lifetime is bounded by the
/// guard, so `ArenaValue<'_>` cannot escape the call).
pub(crate) struct ArenaGuard {
    /// `ManuallyDrop` lets `Drop::drop` move the `Bump` back into the pool
    /// without violating `Drop`'s `&mut self` aliasing rules.
    arena: ManuallyDrop<Bump>,
}

impl ArenaGuard {
    /// Get a `Bump` from the thread-local pool, or allocate a fresh one
    /// sized to `min_capacity` if the pool is empty.
    #[inline]
    pub(crate) fn acquire(min_capacity: usize) -> Self {
        let bump = ARENA_POOL
            .with(|pool| pool.borrow_mut().pop())
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
    fn drop(&mut self) {
        // SAFETY: `arena` is `ManuallyDrop`-wrapped and we only take it once,
        // here, on Drop. After this point `self.arena` is logically uninit
        // and not accessed again.
        let mut bump = unsafe { ManuallyDrop::take(&mut self.arena) };
        bump.reset(); // O(1) — keeps allocated chunks, resets bump pointer

        ARENA_POOL.with(|pool| {
            let mut pool = pool.borrow_mut();
            if pool.len() < POOL_MAX {
                pool.push(bump);
            }
            // else: pool is full, `bump` drops here freeing its chunks
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn acquire_release_reuses_arena() {
        // Drain whatever's in the pool from earlier test runs in this thread.
        ARENA_POOL.with(|p| p.borrow_mut().clear());

        // First acquire: pool empty, fresh Bump.
        let g1 = ArenaGuard::acquire(4096);
        let _ = g1.arena().alloc_str("hello"); // grow to ensure non-trivial state
        drop(g1);

        // Second acquire: should pop the previous one.
        ARENA_POOL.with(|p| {
            assert_eq!(p.borrow().len(), 1, "released arena should be in pool");
        });
        let g2 = ArenaGuard::acquire(4096);
        ARENA_POOL.with(|p| assert_eq!(p.borrow().len(), 0, "acquire popped from pool"));
        drop(g2);

        ARENA_POOL.with(|p| assert_eq!(p.borrow().len(), 1, "back in pool"));
    }

    #[test]
    fn pool_caps_at_max() {
        ARENA_POOL.with(|p| p.borrow_mut().clear());

        // Acquire POOL_MAX + 2 arenas concurrently (all live), then drop them
        // all. The first POOL_MAX should land in the pool; the rest are
        // dropped (their chunks freed).
        let mut guards = Vec::new();
        for _ in 0..(POOL_MAX + 2) {
            let g = ArenaGuard::acquire(1024);
            let _ = g.arena().alloc_str("x");
            guards.push(g);
        }
        drop(guards);

        ARENA_POOL.with(|p| {
            assert_eq!(p.borrow().len(), POOL_MAX, "pool bounded at POOL_MAX");
        });
    }

    #[test]
    fn reset_makes_arena_reusable() {
        ARENA_POOL.with(|p| p.borrow_mut().clear());

        let g1 = ArenaGuard::acquire(4096);
        let s1 = g1.arena().alloc_str("first");
        assert_eq!(s1, "first");
        drop(g1);

        // The popped arena should be reset; allocating again works.
        let g2 = ArenaGuard::acquire(4096);
        let s2 = g2.arena().alloc_str("second");
        assert_eq!(s2, "second");
        drop(g2);
    }
}
