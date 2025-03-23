use crate::arena::DataArena;
use crate::logic::Logic;
use crate::value::DataValue;

use super::comparison_builder::ComparisonBuilder;
use super::arithmetic_builder::ArithmeticBuilder;
use super::control_builder::ControlBuilder;
use super::array_builder::ArrayBuilder;
use super::string_builder::StringBuilder;
use super::variable_builder::VariableBuilder;

/// Main builder for creating JSONLogic rules.
///
/// This is the entry point for the builder API, providing access to different
/// specialized builders for different types of operations.
pub struct RuleBuilder<'a> {
    /// The arena in which all allocations will be made.
    arena: &'a DataArena,
}

impl<'a> RuleBuilder<'a> {
    /// Creates a new rule builder.
    pub fn new(arena: &'a DataArena) -> Self {
        Self { arena }
    }

    /// Returns the arena this builder uses.
    pub fn arena(&self) -> &'a DataArena {
        self.arena
    }

    /// Creates a comparison builder.
    pub fn compare(&self) -> ComparisonBuilder<'a> {
        ComparisonBuilder::new(self.arena)
    }

    /// Creates an arithmetic builder.
    pub fn arithmetic(&self) -> ArithmeticBuilder<'a> {
        ArithmeticBuilder::new(self.arena)
    }

    /// Creates a control flow builder.
    pub fn control(&self) -> ControlBuilder<'a> {
        ControlBuilder::new(self.arena)
    }

    /// Creates an array operation builder.
    pub fn array(&self) -> ArrayBuilder<'a> {
        ArrayBuilder::new(self.arena)
    }

    /// Creates a string operation builder.
    pub fn string_builder(&self) -> StringBuilder<'a> {
        StringBuilder::new(self.arena)
    }

    /// Creates a variable reference.
    pub fn var(&self, path: &str) -> VariableBuilder<'a> {
        VariableBuilder::new(self.arena, path)
    }

    /// Creates a literal value.
    pub fn value<T: Into<DataValue<'a>>>(&self, value: T) -> Logic<'a> {
        Logic::literal(value.into(), self.arena)
    }

    /// Creates a literal boolean value.
    pub fn bool(&self, value: bool) -> Logic<'a> {
        Logic::literal(DataValue::bool(value), self.arena)
    }

    /// Creates a literal integer value.
    pub fn int(&self, value: i64) -> Logic<'a> {
        Logic::literal(DataValue::integer(value), self.arena)
    }

    /// Creates a literal float value.
    pub fn float(&self, value: f64) -> Logic<'a> {
        Logic::literal(DataValue::float(value), self.arena)
    }

    /// Creates a literal string value.
    pub fn string(&self, value: &str) -> Logic<'a> {
        Logic::literal(DataValue::string(self.arena, value), self.arena)
    }

    /// Creates a literal null value.
    pub fn null(&self) -> Logic<'a> {
        Logic::literal(DataValue::null(), self.arena)
    }
    
    /// Creates a logic that gets a variable and returns a default if it doesn't exist.
    pub fn var_with_default(&self, path: &str, default: Logic<'a>) -> Logic<'a> {
        Logic::variable(path, Some(default), self.arena)
    }
} 