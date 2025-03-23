use crate::arena::DataArena;
use crate::logic::Logic;

/// Builder for variable references.
///
/// This builder provides a fluent interface for creating variable references,
/// optionally with default values.
pub struct VariableBuilder<'a> {
    /// The arena in which all allocations will be made.
    arena: &'a DataArena,
    /// The path to the variable.
    path: &'a str,
    /// The default value to use if the variable is not found.
    default: Option<Logic<'a>>,
}

impl<'a> VariableBuilder<'a> {
    /// Creates a new variable builder.
    pub fn new(arena: &'a DataArena, path: &str) -> Self {
        let interned_path = arena.intern_str(path);
        Self {
            arena,
            path: interned_path,
            default: None,
        }
    }

    /// Sets the default value to use if the variable is not found.
    pub fn default(mut self, default: Logic<'a>) -> Self {
        self.default = Some(default);
        self
    }

    /// Builds the variable reference.
    pub fn build(self) -> Logic<'a> {
        Logic::variable(self.path, self.default, self.arena)
    }
}

impl<'a> From<VariableBuilder<'a>> for Logic<'a> {
    fn from(builder: VariableBuilder<'a>) -> Self {
        builder.build()
    }
} 