use crate::arena::DataArena;
use crate::logic::{DateTimeOp, Logic, OperatorType};
use crate::value::DataValue;

/// Builder for datetime operations.
///
/// This builder provides a fluent interface for creating datetime operations
/// such as current time, date formatting, parsing, etc.
pub struct DateTimeBuilder<'a> {
    /// The arena in which all allocations will be made.
    arena: &'a DataArena,
}

impl<'a> DateTimeBuilder<'a> {
    /// Creates a new datetime builder.
    pub fn new(arena: &'a DataArena) -> Self {
        Self { arena }
    }

    /// Creates a now operation that returns the current date and time.
    pub fn now_op(&self) -> Logic<'a> {
        Logic::operator(
            OperatorType::DateTime(DateTimeOp::Now),
            Vec::new(),
            self.arena,
        )
    }

    /// Creates a parse_date operation that parses a date string according to a format.
    pub fn parse_date_op<S, F>(&self, date_str: S, format: F) -> Logic<'a>
    where
        S: Into<Logic<'a>>,
        F: Into<Logic<'a>>,
    {
        Logic::operator(
            OperatorType::DateTime(DateTimeOp::ParseDate),
            vec![date_str.into(), format.into()],
            self.arena,
        )
    }

    /// Creates a parse_date operation with string literals.
    pub fn parse_date(&self, date_str: &str, format: &str) -> Logic<'a> {
        self.parse_date_op(
            Logic::literal(DataValue::string(self.arena, date_str), self.arena),
            Logic::literal(DataValue::string(self.arena, format), self.arena),
        )
    }

    /// Creates a format_date operation that formats a date according to a format string.
    pub fn format_date_op<D, F>(&self, date: D, format: F) -> Logic<'a>
    where
        D: Into<Logic<'a>>,
        F: Into<Logic<'a>>,
    {
        Logic::operator(
            OperatorType::DateTime(DateTimeOp::FormatDate),
            vec![date.into(), format.into()],
            self.arena,
        )
    }

    /// Creates a date_diff operation that calculates the difference between two dates.
    pub fn date_diff_op<D1, D2, U>(&self, date1: D1, date2: D2, unit: U) -> Logic<'a>
    where
        D1: Into<Logic<'a>>,
        D2: Into<Logic<'a>>,
        U: Into<Logic<'a>>,
    {
        Logic::operator(
            OperatorType::DateTime(DateTimeOp::DateDiff),
            vec![date1.into(), date2.into(), unit.into()],
            self.arena,
        )
    }

    /// Creates a date_diff operation with string unit.
    pub fn date_diff<D1, D2>(&self, date1: D1, date2: D2, unit: &str) -> Logic<'a>
    where
        D1: Into<Logic<'a>>,
        D2: Into<Logic<'a>>,
    {
        self.date_diff_op(
            date1.into(),
            date2.into(),
            Logic::literal(DataValue::string(self.arena, unit), self.arena),
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_datetime_builders() {
        let arena = DataArena::new();
        let builder = DateTimeBuilder::new(&arena);

        // Test now operation
        let now_op = builder.now_op();
        assert!(matches!(
            now_op.root().as_operator().unwrap().0,
            OperatorType::DateTime(DateTimeOp::Now)
        ));

        // Test parse_date
        let parse_date = builder.parse_date("2022-07-06", "yyyy-MM-dd");
        assert!(matches!(
            parse_date.root().as_operator().unwrap().0,
            OperatorType::DateTime(DateTimeOp::ParseDate)
        ));

        // Test date_diff
        let date_diff = builder.date_diff(
            Logic::variable("date1", None, &arena),
            Logic::variable("date2", None, &arena),
            "days",
        );
        assert!(matches!(
            date_diff.root().as_operator().unwrap().0,
            OperatorType::DateTime(DateTimeOp::DateDiff)
        ));
    }
}
