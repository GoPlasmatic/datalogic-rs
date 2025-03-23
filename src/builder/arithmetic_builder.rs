use crate::arena::DataArena;
use crate::logic::{Logic, OperatorType};
use crate::ArithmeticOp;

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
    pub fn addOp(&self) -> ArithmeticOperationBuilder<'a> {
        ArithmeticOperationBuilder::new(self.arena, ArithmeticOp::Add)
    }

    /// Creates a subtraction operation.
    pub fn subtractOp(&self) -> ArithmeticOperationBuilder<'a> {
        ArithmeticOperationBuilder::new(self.arena, ArithmeticOp::Subtract)
    }

    /// Creates a multiplication operation.
    pub fn multiplyOp(&self) -> ArithmeticOperationBuilder<'a> {
        ArithmeticOperationBuilder::new(self.arena, ArithmeticOp::Multiply)
    }

    /// Creates a division operation.
    pub fn divideOp(&self) -> ArithmeticOperationBuilder<'a> {
        ArithmeticOperationBuilder::new(self.arena, ArithmeticOp::Divide)
    }

    /// Creates a modulo operation.
    pub fn moduloOp(&self) -> ArithmeticOperationBuilder<'a> {
        ArithmeticOperationBuilder::new(self.arena, ArithmeticOp::Modulo)
    }

    /// Creates a minimum operation.
    pub fn minOp(&self) -> ArithmeticOperationBuilder<'a> {
        ArithmeticOperationBuilder::new(self.arena, ArithmeticOp::Min)
    }

    /// Creates a maximum operation.
    pub fn maxOp(&self) -> ArithmeticOperationBuilder<'a> {
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
    pub fn var(self, path: &str) -> Self {
        let var = Logic::variable(path, None, self.arena);
        self.operand(var)
    }

    /// Adds a literal value as an operand to the arithmetic operation.
    pub fn value<T: Into<crate::value::DataValue<'a>>>(self, value: T) -> Self {
        let val = Logic::literal(value.into(), self.arena);
        self.operand(val)
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
                },
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