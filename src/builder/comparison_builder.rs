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
    pub fn equal(&self) -> ComparisonOperationBuilder<'a> {
        ComparisonOperationBuilder::new(self.arena, ComparisonOp::Equal)
    }

    /// Creates a strict equality comparison.
    pub fn strict_equal(&self) -> ComparisonOperationBuilder<'a> {
        ComparisonOperationBuilder::new(self.arena, ComparisonOp::StrictEqual)
    }

    /// Creates an inequality comparison.
    pub fn not_equal(&self) -> ComparisonOperationBuilder<'a> {
        ComparisonOperationBuilder::new(self.arena, ComparisonOp::NotEqual)
    }

    /// Creates a strict inequality comparison.
    pub fn strict_not_equal(&self) -> ComparisonOperationBuilder<'a> {
        ComparisonOperationBuilder::new(self.arena, ComparisonOp::StrictNotEqual)
    }

    /// Creates a greater than comparison.
    pub fn greater_than(&self) -> ComparisonOperationBuilder<'a> {
        ComparisonOperationBuilder::new(self.arena, ComparisonOp::GreaterThan)
    }

    /// Creates a greater than or equal comparison.
    pub fn greater_than_or_equal(&self) -> ComparisonOperationBuilder<'a> {
        ComparisonOperationBuilder::new(self.arena, ComparisonOp::GreaterThanOrEqual)
    }

    /// Creates a less than comparison.
    pub fn less_than(&self) -> ComparisonOperationBuilder<'a> {
        ComparisonOperationBuilder::new(self.arena, ComparisonOp::LessThan)
    }

    /// Creates a less than or equal comparison.
    pub fn less_than_or_equal(&self) -> ComparisonOperationBuilder<'a> {
        ComparisonOperationBuilder::new(self.arena, ComparisonOp::LessThanOrEqual)
    }
}

/// Builder for a comparison operation with its operands.
pub struct ComparisonOperationBuilder<'a> {
    /// The arena in which all allocations will be made.
    arena: &'a DataArena,
    /// The comparison operator to use.
    operation: ComparisonOp,
    /// The left operand, if set.
    left: Option<Logic<'a>>,
}

impl<'a> ComparisonOperationBuilder<'a> {
    /// Creates a new comparison operation builder.
    pub fn new(arena: &'a DataArena, operation: ComparisonOp) -> Self {
        Self {
            arena,
            operation,
            left: None,
        }
    }

    /// Sets the left operand of the comparison.
    pub fn left(mut self, value: Logic<'a>) -> Self {
        self.left = Some(value);
        self
    }

    /// Sets the left operand of the comparison to a variable.
    pub fn var(self, path: &str) -> Self {
        let var = Logic::variable(path, None, self.arena);
        self.left(var)
    }

    /// Sets the right operand and builds the comparison operation.
    pub fn right(self, right: Logic<'a>) -> Logic<'a> {
        let left = self.left.unwrap_or_else(|| {
            Logic::literal(crate::value::DataValue::null(), self.arena)
        });
        
        Logic::operator(
            OperatorType::Comparison(self.operation),
            vec![left, right],
            self.arena,
        )
    }

    /// Sets the right operand to a variable and builds the comparison operation.
    pub fn var_right(self, path: &str) -> Logic<'a> {
        let right = Logic::variable(path, None, self.arena);
        self.right(right)
    }

    /// Sets the right operand to a literal value and builds the comparison operation.
    pub fn value<T: Into<crate::value::DataValue<'a>>>(self, value: T) -> Logic<'a> {
        let right = Logic::literal(value.into(), self.arena);
        self.right(right)
    }

    /// Builds the comparison operation with the current operands.
    ///
    /// If left operand is not set, it will use null as the default.
    /// If right operand is not set, it will create an "is truthy" check.
    pub fn build(self) -> Logic<'a> {
        let left = self.left.unwrap_or_else(|| {
            Logic::literal(crate::value::DataValue::null(), self.arena)
        });
        
        // For unary operations, we compare with true for a "is truthy" check
        let right = Logic::literal(crate::value::DataValue::bool(true), self.arena);
        
        Logic::operator(
            OperatorType::Comparison(self.operation),
            vec![left, right],
            self.arena,
        )
    }
} 