use crate::arena::DataArena;
use crate::logic::{Logic, OperatorType};
use crate::value::DataValue;

/// Builder for the type operator.
///
/// This builder provides a fluent interface for creating type operators
/// which return the type of a value as a string.
pub struct TypeBuilder<'a> {
    /// The arena in which all allocations will be made.
    arena: &'a DataArena,
}

impl<'a> TypeBuilder<'a> {
    /// Creates a new type builder.
    pub fn new(arena: &'a DataArena) -> Self {
        Self { arena }
    }

    /// Sets the argument for the type operator to a variable reference.
    pub fn var(&self, path: &str) -> TypeOperationBuilder<'a> {
        TypeOperationBuilder::new(self.arena).var(path)
    }

    /// Sets the argument for the type operator to a literal integer.
    pub fn int(&self, value: i64) -> TypeOperationBuilder<'a> {
        TypeOperationBuilder::new(self.arena).int(value)
    }

    /// Sets the argument for the type operator to a literal float.
    pub fn float(&self, value: f64) -> TypeOperationBuilder<'a> {
        TypeOperationBuilder::new(self.arena).float(value)
    }

    /// Sets the argument for the type operator to a literal string.
    pub fn string(&self, value: &str) -> TypeOperationBuilder<'a> {
        TypeOperationBuilder::new(self.arena).string(value)
    }

    /// Sets the argument for the type operator to a literal boolean.
    pub fn bool(&self, value: bool) -> TypeOperationBuilder<'a> {
        TypeOperationBuilder::new(self.arena).bool(value)
    }

    /// Sets the argument for the type operator to a literal null.
    pub fn null(&self) -> TypeOperationBuilder<'a> {
        TypeOperationBuilder::new(self.arena).null()
    }

    /// Sets the argument for the type operator to a literal array.
    pub fn array(&self) -> TypeOperationBuilder<'a> {
        TypeOperationBuilder::new(self.arena).array()
    }

    /// Sets the argument for the type operator to a literal object.
    pub fn object(&self) -> TypeOperationBuilder<'a> {
        TypeOperationBuilder::new(self.arena).object()
    }
}

/// Builder for a type operation with its argument.
pub struct TypeOperationBuilder<'a> {
    /// The arena in which all allocations will be made.
    arena: &'a DataArena,
    /// The argument for the type operator.
    arg: Option<Logic<'a>>,
}

impl<'a> TypeOperationBuilder<'a> {
    /// Creates a new type operation builder.
    pub fn new(arena: &'a DataArena) -> Self {
        Self { arena, arg: None }
    }

    /// Sets the argument for the type operator to a variable reference.
    pub fn var(mut self, path: &str) -> Self {
        self.arg = Some(Logic::variable(path, None, self.arena));
        self
    }

    /// Sets the argument for the type operator to a literal integer.
    pub fn int(mut self, value: i64) -> Self {
        self.arg = Some(Logic::literal(DataValue::integer(value), self.arena));
        self
    }

    /// Sets the argument for the type operator to a literal float.
    pub fn float(mut self, value: f64) -> Self {
        self.arg = Some(Logic::literal(DataValue::float(value), self.arena));
        self
    }

    /// Sets the argument for the type operator to a literal string.
    pub fn string(mut self, value: &str) -> Self {
        self.arg = Some(Logic::literal(
            DataValue::string(self.arena, value),
            self.arena,
        ));
        self
    }

    /// Sets the argument for the type operator to a literal boolean.
    pub fn bool(mut self, value: bool) -> Self {
        self.arg = Some(Logic::literal(DataValue::bool(value), self.arena));
        self
    }

    /// Sets the argument for the type operator to a literal null.
    pub fn null(mut self) -> Self {
        self.arg = Some(Logic::literal(DataValue::null(), self.arena));
        self
    }

    /// Sets the argument for the type operator to a literal empty array.
    pub fn array(mut self) -> Self {
        let empty_array = DataValue::Array(self.arena.vec_into_slice(Vec::new()));
        self.arg = Some(Logic::literal(empty_array, self.arena));
        self
    }

    /// Sets the argument for the type operator to a literal empty object.
    pub fn object(mut self) -> Self {
        let empty_object = DataValue::Object(self.arena.vec_into_slice(Vec::new()));
        self.arg = Some(Logic::literal(empty_object, self.arena));
        self
    }

    /// Sets the argument for the type operator to a provided logic expression.
    pub fn logic(mut self, logic: Logic<'a>) -> Self {
        self.arg = Some(logic);
        self
    }

    /// Builds the type operation logic.
    pub fn build(self) -> Logic<'a> {
        let arg = self.arg.expect("Type operator requires an argument");
        Logic::operator(OperatorType::Type, vec![arg], self.arena)
    }
}
