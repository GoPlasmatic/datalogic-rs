//! Arena allocation system for efficient memory management.
//!
//! This module provides arena-based allocation for the DataLogic library,
//! significantly improving performance by reducing allocation overhead
//! and improving memory locality.

mod bump;
mod custom;

// Re-export the main types
pub use bump::DataArena;

// Re-export the simplified operator types from custom_operator
pub use custom::{CustomOperator, CustomOperatorRegistry, SimpleOperatorAdapter, SimpleOperatorFn};

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_arena_allocation() {
        let arena = DataArena::new();
        let s1 = arena.alloc_str("hello");
        let s2 = arena.alloc_str("world");

        assert_eq!(s1, "hello");
        assert_eq!(s2, "world");

        // Different allocations should yield different references
        assert_ne!(s1.as_ptr(), s2.as_ptr());
    }
}
