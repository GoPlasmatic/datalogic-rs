use crate::arena::DataArena;
use crate::logic::{Logic, OperatorType};
use crate::ComparisonOp;

/// Builder for comparison operations.
///
/// This builder provides a fluent interface for creating comparison operations
/// such as equality, inequality, greater than, etc.
pub struct ComparisonBuilder<'a> {
    /// The arena in which all allocations will be made.
    arena: &'a DataArena,
}

impl<'a> ComparisonBuilder<'a> {
    /// Creates a new comparison builder.
    pub fn new(arena: &'a DataArena) -> Self {
        Self { arena }
    }

    /// Creates an equality comparison.
    pub fn equal_op(&self) -> ComparisonOperationBuilder<'a> {
        ComparisonOperationBuilder::new(self.arena, ComparisonOp::Equal)
    }

    /// Creates a strict equality comparison.
    pub fn strict_equal_op(&self) -> ComparisonOperationBuilder<'a> {
        ComparisonOperationBuilder::new(self.arena, ComparisonOp::StrictEqual)
    }

    /// Creates an inequality comparison.
    pub fn not_equal_op(&self) -> ComparisonOperationBuilder<'a> {
        ComparisonOperationBuilder::new(self.arena, ComparisonOp::NotEqual)
    }

    /// Creates a strict inequality comparison.
    pub fn strict_not_equal_op(&self) -> ComparisonOperationBuilder<'a> {
        ComparisonOperationBuilder::new(self.arena, ComparisonOp::StrictNotEqual)
    }

    /// Creates a greater than comparison.
    pub fn greater_than_op(&self) -> ComparisonOperationBuilder<'a> {
        ComparisonOperationBuilder::new(self.arena, ComparisonOp::GreaterThan)
    }

    /// Creates a greater than or equal comparison.
    pub fn greater_than_or_equal_op(&self) -> ComparisonOperationBuilder<'a> {
        ComparisonOperationBuilder::new(self.arena, ComparisonOp::GreaterThanOrEqual)
    }

    /// Creates a less than comparison.
    pub fn less_than_op(&self) -> ComparisonOperationBuilder<'a> {
        ComparisonOperationBuilder::new(self.arena, ComparisonOp::LessThan)
    }

    /// Creates a less than or equal comparison.
    pub fn less_than_or_equal_op(&self) -> ComparisonOperationBuilder<'a> {
        ComparisonOperationBuilder::new(self.arena, ComparisonOp::LessThanOrEqual)
    }
}

/// Builder for a comparison operation with its operands.
pub struct ComparisonOperationBuilder<'a> {
    /// The arena in which all allocations will be made.
    arena: &'a DataArena,
    /// The comparison operator to use.
    operation: ComparisonOp,
    /// The operands of the comparison.
    operands: Vec<Logic<'a>>,
}

impl<'a> ComparisonOperationBuilder<'a> {
    /// Creates a new comparison operation builder.
    pub fn new(arena: &'a DataArena, operation: ComparisonOp) -> Self {
        Self {
            arena,
            operation,
            operands: Vec::new(),
        }
    }

    /// Adds an operand to the comparison.
    pub fn operand(mut self, value: Logic<'a>) -> Self {
        self.operands.push(value);
        self
    }

    /// Adds a variable operand to the comparison.
    pub fn var(self, path: &str) -> Self {
        let var = Logic::variable(path, None, self.arena);
        self.operand(var)
    }

    /// Adds a literal value operand to the comparison.
    pub fn value<T: Into<crate::value::DataValue<'a>>>(mut self, value: T) -> Self {
        let val = Logic::literal(value.into(), self.arena);
        self.operands.push(val);
        self
    }

    /// Adds an integer value operand to the comparison.
    pub fn int(mut self, value: i64) -> Self {
        let val = Logic::literal(crate::value::DataValue::integer(value), self.arena);
        self.operands.push(val);
        self
    }

    /// Adds a float value operand to the comparison.
    pub fn float(mut self, value: f64) -> Self {
        let val = Logic::literal(crate::value::DataValue::float(value), self.arena);
        self.operands.push(val);
        self
    }

    /// Adds a string value operand to the comparison.
    pub fn string(mut self, value: &str) -> Self {
        let val = Logic::literal(
            crate::value::DataValue::string(self.arena, value),
            self.arena,
        );
        self.operands.push(val);
        self
    }

    /// Adds a boolean value operand to the comparison.
    pub fn bool(mut self, value: bool) -> Self {
        let val = Logic::literal(crate::value::DataValue::bool(value), self.arena);
        self.operands.push(val);
        self
    }

    /// Builds the comparison operation with the current operands.
    ///
    /// If no operands are set, it will use null as the default.
    /// If only one operand is set, it will create an "is truthy" check by comparing with true.
    pub fn build(self) -> Logic<'a> {
        let mut final_operands = self.operands;

        // If no operands are set, use null as the default
        if final_operands.is_empty() {
            final_operands.push(Logic::literal(crate::value::DataValue::null(), self.arena));
        }

        // If only one operand is set, add true as second operand for a "is truthy" check
        if final_operands.len() == 1 {
            final_operands.push(Logic::literal(
                crate::value::DataValue::bool(true),
                self.arena,
            ));
        }

        Logic::operator(
            OperatorType::Comparison(self.operation),
            final_operands,
            self.arena,
        )
    }
}
