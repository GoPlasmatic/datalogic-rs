//! Builder API for creating JSONLogic rules.
//!
//! This module provides a fluent API for building JSONLogic rules in a type-safe manner,
//! with all allocations happening directly in the arena for maximum performance.

mod rule_builder;
mod comparison_builder;
mod arithmetic_builder;
mod control_builder;
mod array_builder;
mod string_builder;
mod variable_builder;
pub mod factory;
#[cfg(test)]
mod tests;

pub use rule_builder::RuleBuilder;
pub use comparison_builder::ComparisonBuilder;
pub use arithmetic_builder::ArithmeticBuilder;
pub use control_builder::ControlBuilder;
pub use array_builder::ArrayBuilder;
pub use string_builder::StringBuilder;
pub use variable_builder::VariableBuilder;
pub use factory::RuleFactory;

use crate::arena::DataArena;

/// Creates a new rule builder that allocates in the provided arena.
///
/// This is the main entry point for the builder API.
pub fn rule_builder<'a>(arena: &'a DataArena) -> RuleBuilder<'a> {
    RuleBuilder::new(arena)
} 