use crate::arena::DataArena;
use crate::logic::ArrayOp;
use crate::logic::{Logic, OperatorType};

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
    pub fn map_op(&self) -> MapBuilder<'a> {
        MapBuilder::new(self.arena)
    }

    /// Creates a filter operation.
    pub fn filter_op(&self) -> FilterBuilder<'a> {
        FilterBuilder::new(self.arena)
    }

    /// Creates a reduce operation.
    pub fn reduce_op(&self) -> ReduceBuilder<'a> {
        ReduceBuilder::new(self.arena)
    }

    /// Creates a merge operation.
    pub fn merge_op(&self) -> ArrayOperationBuilder<'a> {
        ArrayOperationBuilder::new(self.arena, ArrayOp::Merge)
    }

    /// Creates an in-array check operation.
    pub fn in_op(&self, value: Logic<'a>, array: Logic<'a>) -> Logic<'a> {
        Logic::operator(
            OperatorType::Array(ArrayOp::In),
            vec![value, array],
            self.arena,
        )
    }

    /// Creates an array literal.
    pub fn array_literal_op(&self, elements: Vec<Logic<'a>>) -> Logic<'a> {
        Logic::operator(OperatorType::ArrayLiteral, elements, self.arena)
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

    /// Sets the array to map over using a literal array of Logic values.
    pub fn array_literal(self, elements: Vec<Logic<'a>>) -> Self {
        let array = Logic::operator(OperatorType::ArrayLiteral, elements, self.arena);
        self.array(array)
    }

    /// Sets the array to map over using a variable reference.
    pub fn array_var(self, path: &str) -> Self {
        let var = Logic::variable(path, None, self.arena);
        self.array(var)
    }

    /// Sets the mapping function.
    pub fn mapper(mut self, mapper: Logic<'a>) -> Self {
        self.mapper = Some(mapper);
        self
    }

    /// Sets the mapping function using a variable reference.
    pub fn mapper_var(self, path: &str) -> Self {
        let var = Logic::variable(path, None, self.arena);
        self.mapper(var)
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

    /// Sets the array to filter using a literal array of Logic values.
    pub fn array_literal(self, elements: Vec<Logic<'a>>) -> Self {
        let array = Logic::operator(OperatorType::ArrayLiteral, elements, self.arena);
        self.array(array)
    }

    /// Sets the array to filter using a variable reference.
    pub fn array_var(self, path: &str) -> Self {
        let var = Logic::variable(path, None, self.arena);
        self.array(var)
    }

    /// Sets the filter condition.
    pub fn condition(mut self, condition: Logic<'a>) -> Self {
        self.condition = Some(condition);
        self
    }

    /// Sets the filter condition using a variable reference.
    pub fn condition_var(self, path: &str) -> Self {
        let var = Logic::variable(path, None, self.arena);
        self.condition(var)
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

    /// Sets the array to reduce using a literal array of Logic values.
    pub fn array_literal(self, elements: Vec<Logic<'a>>) -> Self {
        let array = Logic::operator(OperatorType::ArrayLiteral, elements, self.arena);
        self.array(array)
    }

    /// Sets the array to reduce using a variable reference.
    pub fn array_var(self, path: &str) -> Self {
        let var = Logic::variable(path, None, self.arena);
        self.array(var)
    }

    /// Sets the reducer function.
    pub fn reducer(mut self, reducer: Logic<'a>) -> Self {
        self.reducer = Some(reducer);
        self
    }

    /// Sets the reducer function using a variable reference.
    pub fn reducer_var(self, path: &str) -> Self {
        let var = Logic::variable(path, None, self.arena);
        self.reducer(var)
    }

    /// Sets the initial value.
    pub fn initial(mut self, initial: Logic<'a>) -> Self {
        self.initial = Some(initial);
        self
    }

    /// Sets the initial value using a variable reference.
    pub fn initial_var(self, path: &str) -> Self {
        let var = Logic::variable(path, None, self.arena);
        self.initial(var)
    }

    /// Sets the initial value as an integer.
    pub fn initial_int(self, value: i64) -> Self {
        let val = Logic::literal(crate::value::DataValue::integer(value), self.arena);
        self.initial(val)
    }

    /// Sets the initial value as a float.
    pub fn initial_float(self, value: f64) -> Self {
        let val = Logic::literal(crate::value::DataValue::float(value), self.arena);
        self.initial(val)
    }

    /// Sets the initial value as a string.
    pub fn initial_string(self, value: &str) -> Self {
        let val = Logic::literal(
            crate::value::DataValue::string(self.arena, value),
            self.arena,
        );
        self.initial(val)
    }

    /// Sets the initial value as a boolean.
    pub fn initial_bool(self, value: bool) -> Self {
        let val = Logic::literal(crate::value::DataValue::bool(value), self.arena);
        self.initial(val)
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
    pub fn operand(mut self, operand: Logic<'a>) -> Self {
        self.operands.push(operand);
        self
    }

    /// Adds a variable as an operand to the array operation.
    pub fn var(mut self, path: &str) -> Self {
        let var = Logic::variable(path, None, self.arena);
        self.operands.push(var);
        self
    }

    /// Adds a literal value as an operand to the array operation.
    pub fn value<T: Into<crate::value::DataValue<'a>>>(mut self, value: T) -> Self {
        let val = Logic::literal(value.into(), self.arena);
        self.operands.push(val);
        self
    }

    /// Adds an integer as an operand to the array operation.
    pub fn int(mut self, value: i64) -> Self {
        let val = Logic::literal(crate::value::DataValue::integer(value), self.arena);
        self.operands.push(val);
        self
    }

    /// Adds a float as an operand to the array operation.
    pub fn float(mut self, value: f64) -> Self {
        let val = Logic::literal(crate::value::DataValue::float(value), self.arena);
        self.operands.push(val);
        self
    }

    /// Adds a string as an operand to the array operation.
    pub fn string(mut self, value: &str) -> Self {
        let val = Logic::literal(
            crate::value::DataValue::string(self.arena, value),
            self.arena,
        );
        self.operands.push(val);
        self
    }

    /// Adds a boolean as an operand to the array operation.
    pub fn bool(mut self, value: bool) -> Self {
        let val = Logic::literal(crate::value::DataValue::bool(value), self.arena);
        self.operands.push(val);
        self
    }

    /// Builds the array operation with the collected operands.
    pub fn build(self) -> Logic<'a> {
        if self.operands.is_empty() {
            return Logic::literal(crate::value::DataValue::array(self.arena, &[]), self.arena);
        }

        Logic::operator(
            OperatorType::Array(self.operation),
            self.operands,
            self.arena,
        )
    }
}
