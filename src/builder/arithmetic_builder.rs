use crate::arena::DataArena;
use crate::logic::{Logic, OperatorType};
use crate::logic::ArithmeticOp;

/// Builder for arithmetic operations.
///
/// This builder provides a fluent interface for creating arithmetic operations
/// such as addition, subtraction, multiplication, etc.
pub struct ArithmeticBuilder<'a> {
    /// The arena in which all allocations will be made.
    arena: &'a DataArena,
}

impl<'a> ArithmeticBuilder<'a> {
    /// Creates a new arithmetic builder.
    pub fn new(arena: &'a DataArena) -> Self {
        Self { arena }
    }

    /// Creates an addition operation.
    pub fn add_op(&self) -> ArithmeticOperationBuilder<'a> {
        ArithmeticOperationBuilder::new(self.arena, ArithmeticOp::Add)
    }

    /// Creates a subtraction operation.
    pub fn subtract_op(&self) -> ArithmeticOperationBuilder<'a> {
        ArithmeticOperationBuilder::new(self.arena, ArithmeticOp::Subtract)
    }

    /// Creates a multiplication operation.
    pub fn multiply_op(&self) -> ArithmeticOperationBuilder<'a> {
        ArithmeticOperationBuilder::new(self.arena, ArithmeticOp::Multiply)
    }

    /// Creates a division operation.
    pub fn divide_op(&self) -> ArithmeticOperationBuilder<'a> {
        ArithmeticOperationBuilder::new(self.arena, ArithmeticOp::Divide)
    }

    /// Creates a modulo operation.
    pub fn modulo_op(&self) -> ArithmeticOperationBuilder<'a> {
        ArithmeticOperationBuilder::new(self.arena, ArithmeticOp::Modulo)
    }

    /// Creates a minimum operation.
    pub fn min_op(&self) -> ArithmeticOperationBuilder<'a> {
        ArithmeticOperationBuilder::new(self.arena, ArithmeticOp::Min)
    }

    /// Creates a maximum operation.
    pub fn max_op(&self) -> ArithmeticOperationBuilder<'a> {
        ArithmeticOperationBuilder::new(self.arena, ArithmeticOp::Max)
    }
}

/// Builder for an arithmetic operation with its operands.
pub struct ArithmeticOperationBuilder<'a> {
    /// The arena in which all allocations will be made.
    arena: &'a DataArena,
    /// The arithmetic operation to use.
    operation: ArithmeticOp,
    /// The operands collected so far.
    operands: Vec<Logic<'a>>,
}

impl<'a> ArithmeticOperationBuilder<'a> {
    /// Creates a new arithmetic operation builder.
    pub fn new(arena: &'a DataArena, operation: ArithmeticOp) -> Self {
        Self {
            arena,
            operation,
            operands: Vec::new(),
        }
    }

    /// Adds an operand to the arithmetic operation.
    pub fn operand(mut self, operand: Logic<'a>) -> Self {
        self.operands.push(operand);
        self
    }

    /// Adds a variable as an operand to the arithmetic operation.
    pub fn var(mut self, path: &str) -> Self {
        let var = Logic::variable(path, None, self.arena);
        self.operands.push(var);
        self
    }

    /// Adds a literal value as an operand to the arithmetic operation.
    pub fn value<T: Into<crate::value::DataValue<'a>>>(mut self, value: T) -> Self {
        let val = Logic::literal(value.into(), self.arena);
        self.operands.push(val);
        self
    }

    /// Adds an integer value as an operand to the arithmetic operation.
    pub fn int(mut self, value: i64) -> Self {
        let val = Logic::literal(crate::value::DataValue::integer(value), self.arena);
        self.operands.push(val);
        self
    }

    /// Adds a float value as an operand to the arithmetic operation.
    pub fn float(mut self, value: f64) -> Self {
        let val = Logic::literal(crate::value::DataValue::float(value), self.arena);
        self.operands.push(val);
        self
    }

    /// Adds a string value as an operand to the arithmetic operation.
    pub fn string(mut self, value: &str) -> Self {
        let val = Logic::literal(
            crate::value::DataValue::string(self.arena, value),
            self.arena,
        );
        self.operands.push(val);
        self
    }

    /// Adds a boolean value as an operand to the arithmetic operation.
    pub fn bool(mut self, value: bool) -> Self {
        let val = Logic::literal(crate::value::DataValue::bool(value), self.arena);
        self.operands.push(val);
        self
    }

    /// Builds the arithmetic operation with the collected operands.
    ///
    /// If no operands have been added, it will use appropriate defaults:
    /// - For addition: 0
    /// - For multiplication: 1
    /// - For others: 0
    pub fn build(self) -> Logic<'a> {
        if self.operands.is_empty() {
            // Default for different operations
            let default_value = match self.operation {
                ArithmeticOp::Add => 0i64,
                ArithmeticOp::Multiply => 1i64,
                _ => 0i64,
            };
            return Logic::literal(crate::value::DataValue::integer(default_value), self.arena);
        }

        // For unary operations, handle them specially
        if self.operands.len() == 1 {
            match self.operation {
                ArithmeticOp::Subtract => {
                    // Unary minus: [-, x] means -x (negate)
                    let zero = Logic::literal(crate::value::DataValue::integer(0), self.arena);
                    return Logic::operator(
                        OperatorType::Arithmetic(self.operation),
                        vec![zero, self.operands[0].clone()],
                        self.arena,
                    );
                }
                _ => {
                    // For other operations, just return the operand for unary case
                    return self.operands[0].clone();
                }
            }
        }

        Logic::operator(
            OperatorType::Arithmetic(self.operation),
            self.operands,
            self.arena,
        )
    }
}
