use crate::arena::DataArena;
use crate::logic::{Logic, OperatorType};
use crate::logic::ArrayOp;

/// Builder for array operations.
///
/// This builder provides a fluent interface for creating array operations
/// such as map, filter, reduce, etc.
pub struct ArrayBuilder<'a> {
    /// The arena in which all allocations will be made.
    arena: &'a DataArena,
}

impl<'a> ArrayBuilder<'a> {
    /// Creates a new array builder.
    pub fn new(arena: &'a DataArena) -> Self {
        Self { arena }
    }

    /// Creates a map operation.
    pub fn map(&self) -> MapBuilder<'a> {
        MapBuilder::new(self.arena)
    }

    /// Creates a filter operation.
    pub fn filter(&self) -> FilterBuilder<'a> {
        FilterBuilder::new(self.arena)
    }

    /// Creates a reduce operation.
    pub fn reduce(&self) -> ReduceBuilder<'a> {
        ReduceBuilder::new(self.arena)
    }

    /// Creates a merge operation.
    pub fn merge(&self) -> ArrayOperationBuilder<'a> {
        ArrayOperationBuilder::new(self.arena, ArrayOp::Merge)
    }

    /// Creates an in-array check operation.
    pub fn in_array(&self, value: Logic<'a>, array: Logic<'a>) -> Logic<'a> {
        Logic::operator(
            OperatorType::In,
            vec![value, array],
            self.arena,
        )
    }

    /// Creates an array literal.
    pub fn array_literal(&self, elements: Vec<Logic<'a>>) -> Logic<'a> {
        Logic::operator(
            OperatorType::ArrayLiteral,
            elements,
            self.arena,
        )
    }
}

/// Builder for map operations.
pub struct MapBuilder<'a> {
    /// The arena in which all allocations will be made.
    arena: &'a DataArena,
    /// The array to map over.
    array: Option<Logic<'a>>,
    /// The mapping function.
    mapper: Option<Logic<'a>>,
}

impl<'a> MapBuilder<'a> {
    /// Creates a new map builder.
    pub fn new(arena: &'a DataArena) -> Self {
        Self {
            arena,
            array: None,
            mapper: None,
        }
    }

    /// Sets the array to map over.
    pub fn array(mut self, array: Logic<'a>) -> Self {
        self.array = Some(array);
        self
    }

    /// Sets the mapping function.
    pub fn mapper(mut self, mapper: Logic<'a>) -> Self {
        self.mapper = Some(mapper);
        self
    }

    /// Builds the map operation.
    pub fn build(self) -> Logic<'a> {
        let array = self.array.unwrap_or_else(|| {
            Logic::literal(crate::value::DataValue::array(self.arena, &[]), self.arena)
        });
        
        let mapper = self.mapper.unwrap_or_else(|| {
            // Default mapper is identity function
            Logic::variable("", None, self.arena)
        });
        
        Logic::operator(
            OperatorType::Array(ArrayOp::Map),
            vec![array, mapper],
            self.arena,
        )
    }
}

/// Builder for filter operations.
pub struct FilterBuilder<'a> {
    /// The arena in which all allocations will be made.
    arena: &'a DataArena,
    /// The array to filter.
    array: Option<Logic<'a>>,
    /// The filter condition.
    condition: Option<Logic<'a>>,
}

impl<'a> FilterBuilder<'a> {
    /// Creates a new filter builder.
    pub fn new(arena: &'a DataArena) -> Self {
        Self {
            arena,
            array: None,
            condition: None,
        }
    }

    /// Sets the array to filter.
    pub fn array(mut self, array: Logic<'a>) -> Self {
        self.array = Some(array);
        self
    }

    /// Sets the filter condition.
    pub fn condition(mut self, condition: Logic<'a>) -> Self {
        self.condition = Some(condition);
        self
    }

    /// Builds the filter operation.
    pub fn build(self) -> Logic<'a> {
        let array = self.array.unwrap_or_else(|| {
            Logic::literal(crate::value::DataValue::array(self.arena, &[]), self.arena)
        });
        
        let condition = self.condition.unwrap_or_else(|| {
            // Default condition is truthy check
            Logic::variable("", None, self.arena)
        });
        
        Logic::operator(
            OperatorType::Array(ArrayOp::Filter),
            vec![array, condition],
            self.arena,
        )
    }
}

/// Builder for reduce operations.
pub struct ReduceBuilder<'a> {
    /// The arena in which all allocations will be made.
    arena: &'a DataArena,
    /// The array to reduce.
    array: Option<Logic<'a>>,
    /// The reducer function.
    reducer: Option<Logic<'a>>,
    /// The initial value.
    initial: Option<Logic<'a>>,
}

impl<'a> ReduceBuilder<'a> {
    /// Creates a new reduce builder.
    pub fn new(arena: &'a DataArena) -> Self {
        Self {
            arena,
            array: None,
            reducer: None,
            initial: None,
        }
    }

    /// Sets the array to reduce.
    pub fn array(mut self, array: Logic<'a>) -> Self {
        self.array = Some(array);
        self
    }

    /// Sets the reducer function.
    pub fn reducer(mut self, reducer: Logic<'a>) -> Self {
        self.reducer = Some(reducer);
        self
    }

    /// Sets the initial value.
    pub fn initial(mut self, initial: Logic<'a>) -> Self {
        self.initial = Some(initial);
        self
    }

    /// Builds the reduce operation.
    pub fn build(self) -> Logic<'a> {
        let array = self.array.unwrap_or_else(|| {
            Logic::literal(crate::value::DataValue::array(self.arena, &[]), self.arena)
        });
        
        let reducer = self.reducer.unwrap_or_else(|| {
            // Default reducer is sum
            let var_a = Logic::variable("current", None, self.arena);
            let var_b = Logic::variable("accumulator", None, self.arena);
            Logic::operator(
                OperatorType::Arithmetic(crate::logic::ArithmeticOp::Add),
                vec![var_a, var_b],
                self.arena,
            )
        });
        
        let initial = self.initial.unwrap_or_else(|| {
            // Default initial value is 0
            Logic::literal(crate::value::DataValue::integer(0), self.arena)
        });
        
        Logic::operator(
            OperatorType::Array(ArrayOp::Reduce),
            vec![array, reducer, initial],
            self.arena,
        )
    }
}

/// Builder for generic array operations like merge.
pub struct ArrayOperationBuilder<'a> {
    /// The arena in which all allocations will be made.
    arena: &'a DataArena,
    /// The array operation to use.
    operation: ArrayOp,
    /// The operands collected so far.
    operands: Vec<Logic<'a>>,
}

impl<'a> ArrayOperationBuilder<'a> {
    /// Creates a new array operation builder.
    pub fn new(arena: &'a DataArena, operation: ArrayOp) -> Self {
        Self {
            arena,
            operation,
            operands: Vec::new(),
        }
    }

    /// Adds an operand to the array operation.
    pub fn add(mut self, operand: Logic<'a>) -> Self {
        self.operands.push(operand);
        self
    }

    /// Adds a variable as an operand to the array operation.
    pub fn var(self, path: &str) -> Self {
        let var = Logic::variable(path, None, self.arena);
        self.add(var)
    }

    /// Adds a literal value as an operand to the array operation.
    pub fn value<T: Into<crate::value::DataValue<'a>>>(self, value: T) -> Self {
        let val = Logic::literal(value.into(), self.arena);
        self.add(val)
    }

    /// Builds the array operation with the collected operands.
    pub fn build(self) -> Logic<'a> {
        if self.operands.is_empty() {
            // Default for array operations is an empty array
            return Logic::literal(crate::value::DataValue::array(self.arena, &[]), self.arena);
        }

        Logic::operator(
            OperatorType::Array(self.operation),
            self.operands,
            self.arena,
        )
    }
} 