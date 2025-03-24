//! Builder API for creating JSONLogic rules.
//!
//! This module provides a fluent API for building JSONLogic rules in a type-safe manner,
//! with all allocations happening directly in the arena for maximum performance.

mod arithmetic_builder;
mod array_builder;
mod comparison_builder;
mod control_builder;
pub mod factory;
mod rule_builder;
mod string_builder;
#[cfg(test)]
mod tests;
mod variable_builder;

pub use arithmetic_builder::ArithmeticBuilder;
pub use array_builder::ArrayBuilder;
pub use comparison_builder::ComparisonBuilder;
pub use control_builder::ControlBuilder;
pub use factory::RuleFactory;
pub use rule_builder::RuleBuilder;
pub use string_builder::StringBuilder;
pub use variable_builder::VariableBuilder;

use crate::arena::DataArena;

/// Creates a new rule builder that allocates in the provided arena.
///
/// This is the main entry point for the builder API.
pub fn rule_builder(arena: &DataArena) -> RuleBuilder<'_> {
    RuleBuilder::new(arena)
}
