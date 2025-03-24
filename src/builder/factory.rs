use crate::arena::DataArena;
use crate::logic::Logic;
use crate::value::DataValue;

/// Factory for common rule patterns.
///
/// This provides a set of factory methods for creating common rule patterns
/// without needing to manually construct them using the builder API.
pub struct RuleFactory<'a> {
    /// The arena in which all allocations will be made.
    arena: &'a DataArena,
}

impl<'a> RuleFactory<'a> {
    /// Creates a new rule factory.
    pub fn new(arena: &'a DataArena) -> Self {
        Self { arena }
    }

    /// Creates a between rule (exclusive).
    ///
    /// This creates a rule that checks if a value is between min and max (exclusive).
    pub fn between_exclusive(&self, var_path: &str, min: Logic<'a>, max: Logic<'a>) -> Logic<'a> {
        let builder = super::rule_builder(self.arena);

        builder
            .control()
            .and_op()
            .operand(
                builder
                    .compare()
                    .greater_than_op()
                    .var(var_path)
                    .operand(min)
                    .build(),
            )
            .operand(
                builder
                    .compare()
                    .less_than_op()
                    .var(var_path)
                    .operand(max)
                    .build(),
            )
            .build()
    }

    /// Creates a between rule (inclusive).
    ///
    /// This creates a rule that checks if a value is between min and max (inclusive).
    pub fn between_inclusive(&self, var_path: &str, min: Logic<'a>, max: Logic<'a>) -> Logic<'a> {
        let builder = super::rule_builder(self.arena);

        builder
            .control()
            .and_op()
            .operand(
                builder
                    .compare()
                    .greater_than_or_equal_op()
                    .var(var_path)
                    .operand(min)
                    .build(),
            )
            .operand(
                builder
                    .compare()
                    .less_than_or_equal_op()
                    .var(var_path)
                    .operand(max)
                    .build(),
            )
            .build()
    }

    /// Creates an is-one-of rule.
    ///
    /// This creates a rule that checks if a value is one of a list of options.
    pub fn is_one_of(&self, var_path: &str, options: Vec<impl Into<DataValue<'a>>>) -> Logic<'a> {
        let builder = super::rule_builder(self.arena);
        let var = builder.var(var_path).build();

        // Convert options to Logic rules
        let options_array = options
            .into_iter()
            .map(|opt| builder.value(opt.into()))
            .collect::<Vec<_>>();

        let array = builder.array().array_literal_op(options_array);

        builder.array().in_op(var, array)
    }

    /// Creates a conditional assignment rule.
    ///
    /// This creates a rule that assigns one of two values based on a condition.
    pub fn conditional_value(
        &self,
        condition: Logic<'a>,
        true_value: impl Into<DataValue<'a>>,
        false_value: impl Into<DataValue<'a>>,
    ) -> Logic<'a> {
        let builder = super::rule_builder(self.arena);

        builder
            .control()
            .if_op()
            .condition(condition)
            .then(builder.value(true_value))
            .else_branch(builder.value(false_value))
            .build()
    }

    /// Creates a mapped values rule.
    ///
    /// This creates a rule that maps a variable to different values
    /// based on a mapping table.
    pub fn mapped_value<T: Into<DataValue<'a>> + Clone>(
        &self,
        var_path: &str,
        mapping: &[(T, T)],
        default_value: Option<T>,
    ) -> Logic<'a> {
        let builder = super::rule_builder(self.arena);

        // Start building a chain of if-then-else
        let mut current_rule = None;

        let var = builder.var(var_path).build();

        // Build the chain in reverse (from last to first)
        for (key, value) in mapping.iter().rev() {
            let comparison = builder
                .compare()
                .equal_op()
                .var(var_path)
                .operand(builder.value(key.clone()))
                .build();

            if let Some(else_rule) = current_rule {
                current_rule = Some(
                    builder
                        .control()
                        .if_op()
                        .condition(comparison)
                        .then(builder.value(value.clone()))
                        .else_branch(else_rule)
                        .build(),
                );
            } else {
                // This is the last mapping pair, so use default if provided
                let else_value = if let Some(ref default) = default_value {
                    builder.value(default.clone())
                } else {
                    var.clone() // If no default, return the original value
                };

                current_rule = Some(
                    builder
                        .control()
                        .if_op()
                        .condition(comparison)
                        .then(builder.value(value.clone()))
                        .else_branch(else_value)
                        .build(),
                );
            }
        }

        current_rule.unwrap_or(var)
    }

    /// Creates a string template rule.
    ///
    /// This creates a rule that concatenates strings and variable values according to a template.
    /// The template_parts parameter is a vector of (is_var, value) pairs, where is_var indicates
    /// whether the value is a variable path (true) or a literal string (false).
    pub fn string_template(&self, template_parts: Vec<(bool, &str)>) -> Logic<'a> {
        let builder = super::rule_builder(self.arena);

        if template_parts.is_empty() {
            return builder.string_value("");
        }

        let mut concat = builder.string_ops().concat_op();

        for (is_var, value) in template_parts {
            if is_var {
                concat = concat.operand(builder.var(value).build());
            } else {
                concat = concat.operand(builder.string_value(value));
            }
        }

        concat.build()
    }

    /// Creates a coalesce rule.
    ///
    /// This creates a rule that returns the first non-missing variable value from a list.
    pub fn coalesce(&self, vars: Vec<&str>) -> Logic<'a> {
        let builder = super::rule_builder(self.arena);

        if vars.is_empty() {
            return builder.null();
        }

        if vars.len() == 1 {
            return builder.var(vars[0]).build();
        }

        // For multiple variables, we need to create a chain of if-else checks
        let var_path = vars[0];
        let var = builder.var(var_path).build();

        // Check if the first variable is missing
        let is_missing = builder.missing_op(builder.string_value(var_path));

        // If it's missing, coalesce the rest of the variables
        let rest_coalesce = self.coalesce(vars[1..].to_vec());

        builder
            .control()
            .if_op()
            .condition(is_missing)
            .then(rest_coalesce)
            .else_branch(var)
            .build()
    }

    /// Creates a rule for validating that a variable is within an inclusive range.
    pub fn validate_in_range(&self, var_path: &str, min: Logic<'a>, max: Logic<'a>) -> Logic<'a> {
        let builder = super::rule_builder(self.arena);

        builder
            .control()
            .and_op()
            .operand(
                builder
                    .compare()
                    .greater_than_op()
                    .var(var_path)
                    .operand(min)
                    .build(),
            )
            .operand(
                builder
                    .compare()
                    .less_than_op()
                    .var(var_path)
                    .operand(max)
                    .build(),
            )
            .build()
    }

    /// Creates a rule for validating that a variable is within an inclusive range.
    pub fn validate_in_range_inclusive(
        &self,
        var_path: &str,
        min: Logic<'a>,
        max: Logic<'a>,
    ) -> Logic<'a> {
        let builder = super::rule_builder(self.arena);

        builder
            .control()
            .and_op()
            .operand(
                builder
                    .compare()
                    .greater_than_or_equal_op()
                    .var(var_path)
                    .operand(min)
                    .build(),
            )
            .operand(
                builder
                    .compare()
                    .less_than_or_equal_op()
                    .var(var_path)
                    .operand(max)
                    .build(),
            )
            .build()
    }

    /// Creates a rule for validating that a variable is one of a set of options.
    pub fn validate_in_options(&self, var_path: &str, options: &[Logic<'a>]) -> Logic<'a> {
        let builder = super::rule_builder(self.arena);

        let options_array = options.to_vec();
        let array = builder.array().array_literal_op(options_array);
        let var = builder.var(var_path).build();

        builder.array().in_op(var, array)
    }

    /// Creates a rule for validating a field conditionally based on another field.
    pub fn validate_if(
        &self,
        condition: Logic<'a>,
        then_rule: Logic<'a>,
        else_rule: Logic<'a>,
    ) -> Logic<'a> {
        let builder = super::rule_builder(self.arena);

        builder
            .control()
            .if_op()
            .condition(condition)
            .then(then_rule)
            .else_branch(else_rule)
            .build()
    }

    /// Creates a rule for validating that an object has all the required fields.
    pub fn validate_required_fields(&self, fields: &[&str]) -> Logic<'a> {
        let builder = super::rule_builder(self.arena);

        let mut all_fields_present =
            Logic::literal(crate::value::DataValue::bool(true), self.arena);

        for field in fields {
            let comparison = builder
                .compare()
                .equal_op()
                .operand(
                    builder
                        .control()
                        .if_op()
                        .condition(builder.missing_var(field))
                        .then(builder.bool(false))
                        .else_branch(builder.bool(true))
                        .build(),
                )
                .operand(builder.bool(true))
                .build();

            let new_value = if matches!(all_fields_present.as_literal(), Some(v) if v.as_bool() == Some(true))
            {
                comparison
            } else {
                builder
                    .control()
                    .if_op()
                    .condition(comparison)
                    .then(all_fields_present)
                    .else_branch(builder.bool(false))
                    .build()
            };

            all_fields_present = new_value;
        }

        all_fields_present
    }

    /// Creates a rule for concatenating variables into a string.
    pub fn concat_vars(&self, vars: &[&str], separator: &str) -> Logic<'a> {
        let builder = super::rule_builder(self.arena);

        let mut concat = builder.string_ops().concat_op();

        for (i, var) in vars.iter().enumerate() {
            if i > 0 {
                concat = concat.string(separator);
            }
            concat = concat.operand(builder.var(var).build());
        }

        concat.build()
    }

    /// Creates a rule for handling missing variables by using a default value.
    pub fn default_if_missing(&self, var_path: &str, default_value: Logic<'a>) -> Logic<'a> {
        let builder = super::rule_builder(self.arena);

        let is_missing = builder.missing_op(builder.string_value(var_path));

        let var = builder.var(var_path).build();

        builder
            .control()
            .if_op()
            .condition(is_missing)
            .then(default_value)
            .else_branch(var)
            .build()
    }
}
