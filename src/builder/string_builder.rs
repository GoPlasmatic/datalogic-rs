use crate::arena::DataArena;
use crate::logic::{Logic, OperatorType};
use crate::logic::StringOp;

/// Builder for string operations.
///
/// This builder provides a fluent interface for creating string operations
/// such as concatenation, substring, etc.
pub struct StringBuilder<'a> {
    /// The arena in which all allocations will be made.
    arena: &'a DataArena,
}

impl<'a> StringBuilder<'a> {
    /// Creates a new string builder.
    pub fn new(arena: &'a DataArena) -> Self {
        Self { arena }
    }

    /// Creates a concatenation operation.
    pub fn concatOp(&self) -> StringOperationBuilder<'a> {
        StringOperationBuilder::new(self.arena, StringOp::Cat)
    }

    /// Creates a substring operation.
    pub fn substrOp(&self) -> SubstringBuilder<'a> {
        SubstringBuilder::new(self.arena)
    }
}

/// Builder for string operations with multiple operands.
pub struct StringOperationBuilder<'a> {
    /// The arena in which all allocations will be made.
    arena: &'a DataArena,
    /// The string operation to use.
    operation: StringOp,
    /// The operands collected so far.
    operands: Vec<Logic<'a>>,
}

impl<'a> StringOperationBuilder<'a> {
    /// Creates a new string operation builder.
    pub fn new(arena: &'a DataArena, operation: StringOp) -> Self {
        Self {
            arena,
            operation,
            operands: Vec::new(),
        }
    }

    /// Adds a part to the string operation.
    pub fn part(mut self, operand: Logic<'a>) -> Self {
        self.operands.push(operand);
        self
    }

    /// Adds a variable as a part to the string operation.
    pub fn var(mut self, path: &str) -> Self {
        let var = Logic::variable(path, None, self.arena);
        self.operands.push(var);
        self
    }

    /// Adds a literal string value as a part to the string operation.
    pub fn string(mut self, value: &str) -> Self {
        let val = Logic::literal(crate::value::DataValue::string(self.arena, value), self.arena);
        self.operands.push(val);
        self
    }

    /// Adds an integer as a part to the string operation.
    pub fn int(mut self, value: i64) -> Self {
        let val = Logic::literal(crate::value::DataValue::integer(value), self.arena);
        self.operands.push(val);
        self
    }

    /// Adds a float as a part to the string operation.
    pub fn float(mut self, value: f64) -> Self {
        let val = Logic::literal(crate::value::DataValue::float(value), self.arena);
        self.operands.push(val);
        self
    }

    /// Adds a boolean as a part to the string operation.
    pub fn bool(mut self, value: bool) -> Self {
        let val = Logic::literal(crate::value::DataValue::bool(value), self.arena);
        self.operands.push(val);
        self
    }

    /// Builds the string operation with the collected parts.
    pub fn build(self) -> Logic<'a> {
        if self.operands.is_empty() {
            // Default for string operations is an empty string
            return Logic::literal(crate::value::DataValue::string(self.arena, ""), self.arena);
        }

        Logic::operator(
            OperatorType::String(self.operation),
            self.operands,
            self.arena,
        )
    }
}

/// Builder for substring operations.
pub struct SubstringBuilder<'a> {
    /// The arena in which all allocations will be made.
    arena: &'a DataArena,
    /// The string to extract from.
    string: Option<Logic<'a>>,
    /// The start index.
    start: Option<Logic<'a>>,
    /// The length.
    length: Option<Logic<'a>>,
}

impl<'a> SubstringBuilder<'a> {
    /// Creates a new substring builder.
    pub fn new(arena: &'a DataArena) -> Self {
        Self {
            arena,
            string: None,
            start: None,
            length: None,
        }
    }

    /// Sets the string to extract from.
    pub fn string(mut self, string: Logic<'a>) -> Self {
        self.string = Some(string);
        self
    }

    /// Sets the string to extract from using a variable reference.
    pub fn var(self, path: &str) -> Self {
        let var = Logic::variable(path, None, self.arena);
        self.string(var)
    }

    /// Sets the string to extract from using a literal string.
    pub fn literal(self, value: &str) -> Self {
        let val = Logic::literal(crate::value::DataValue::string(self.arena, value), self.arena);
        self.string(val)
    }

    /// Sets the start index.
    pub fn start(mut self, start: Logic<'a>) -> Self {
        self.start = Some(start);
        self
    }

    /// Sets the start index using an integer literal.
    pub fn start_at(self, index: i64) -> Self {
        let val = Logic::literal(crate::value::DataValue::integer(index), self.arena);
        self.start(val)
    }

    /// Sets the length.
    pub fn length(mut self, length: Logic<'a>) -> Self {
        self.length = Some(length);
        self
    }

    /// Sets the length using an integer literal.
    pub fn take(self, length: i64) -> Self {
        let val = Logic::literal(crate::value::DataValue::integer(length), self.arena);
        self.length(val)
    }

    /// Builds the substring operation.
    ///
    /// If string is not set, it will use an empty string.
    /// If start is not set, it will use 0.
    /// If length is not set, it will extract to the end of the string.
    pub fn build(self) -> Logic<'a> {
        let string = self.string.unwrap_or_else(|| {
            Logic::literal(crate::value::DataValue::string(self.arena, ""), self.arena)
        });
        
        let start = self.start.unwrap_or_else(|| {
            Logic::literal(crate::value::DataValue::integer(0), self.arena)
        });
        
        let mut operands = vec![string, start];
        
        if let Some(length) = self.length {
            operands.push(length);
        }
        
        Logic::operator(
            OperatorType::String(StringOp::Substr),
            operands,
            self.arena,
        )
    }
} 