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
    pub fn string_ops(&self) -> StringBuilder<'a> {
        StringBuilder::new(self.arena)
    }

    /// Creates a variable reference.
    pub fn var(&self, path: &str) -> VariableBuilder<'a> {
        VariableBuilder::new(self.arena, path)
    }

    /// Creates a val token with the given path components.
    /// The path can be a string, number, or array of components.
    pub fn valOp<T: Into<DataValue<'a>>>(&self, path: T) -> Logic<'a> {
        let path_value = path.into();
        let path_logic = Logic::literal(path_value, self.arena);
        Logic::operator(crate::logic::OperatorType::Val, vec![path_logic], self.arena)
    }
    
    /// Creates a val token with a string path.
    pub fn val_str(&self, path: &str) -> Logic<'a> {
        self.valOp(DataValue::string(self.arena, path))
    }
    
    /// Creates a val token with an array of path components.
    /// Each component can be a string or number for array indices.
    pub fn val_path<I, T>(&self, components: I) -> Logic<'a> 
    where 
        I: IntoIterator<Item = T>,
        T: Into<DataValue<'a>>,
    {
        let mut path_components = Vec::new();
        for component in components {
            path_components.push(component.into());
        }
        let array_value = DataValue::Array(self.arena.alloc_slice_clone(&path_components));
        self.valOp(array_value)
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
    pub fn string_value(&self, value: &str) -> Logic<'a> {
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
    
    /// Creates a missing check for the specified variables.
    /// Returns an array of variables that are missing from the data context.
    pub fn missingOp<T: Into<Logic<'a>>>(&self, variables: T) -> Logic<'a> {
        let vars = variables.into();
        Logic::operator(crate::logic::OperatorType::Missing, vec![vars], self.arena)
    }
    
    /// Creates a missing check for a single variable.
    pub fn missing_var(&self, variable: &str) -> Logic<'a> {
        self.missingOp(self.string_value(variable))
    }
    
    /// Creates a missing check for multiple variables.
    pub fn missing_vars<I, S>(&self, variables: I) -> Logic<'a>
    where
        I: IntoIterator<Item = S>,
        S: AsRef<str>,
    {
        let vars = variables.into_iter()
            .map(|s| self.string_value(s.as_ref()))
            .collect::<Vec<_>>();
        
        self.missingOp(self.array().arrayLiteralOp(vars))
    }
    
    /// Creates a missing_some check, which returns an array of variables that 
    /// are missing from the data context if the number of present variables is
    /// less than the required number.
    pub fn missingSomeOp<T: Into<Logic<'a>>>(&self, min_required: i64, variables: T) -> Logic<'a> {
        let vars = variables.into();
        let min = self.int(min_required);
        
        Logic::operator(crate::logic::OperatorType::MissingSome, vec![min, vars], self.arena)
    }
    
    /// Creates a throw operator that throws an error with the given value.
    pub fn throwOp<T: Into<Logic<'a>>>(&self, error: T) -> Logic<'a> {
        let error_value = error.into();
        Logic::operator(crate::logic::OperatorType::Throw, vec![error_value], self.arena)
    }
    
    /// Creates a try operator that attempts to evaluate a sequence of expressions.
    /// Returns the result of the first one that succeeds. If all expressions fail,
    /// the last error is propagated.
    pub fn tryOp<I, T>(&self, expressions: I) -> Logic<'a>
    where
        I: IntoIterator<Item = T>,
        T: Into<Logic<'a>>,
    {
        let expressions = expressions.into_iter()
            .map(|expr| expr.into())
            .collect::<Vec<_>>();
        
        Logic::operator(crate::logic::OperatorType::Try, expressions, self.arena)
    }
} 