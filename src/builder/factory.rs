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
        let var = builder.var(var_path).build();
        
        builder
            .control()
            .andOp()
            .operand(builder.compare().greaterThanOp().left(var.clone()).right(min))
            .operand(builder.compare().lessThanOp().left(var).right(max))
            .build()
    }
    
    /// Creates a between rule (inclusive).
    ///
    /// This creates a rule that checks if a value is between min and max (inclusive).
    pub fn between_inclusive(&self, var_path: &str, min: Logic<'a>, max: Logic<'a>) -> Logic<'a> {
        let builder = super::rule_builder(self.arena);
        let var = builder.var(var_path).build();
        
        builder
            .control()
            .andOp()
            .operand(builder.compare().greaterThanOrEqualOp().left(var.clone()).right(min))
            .operand(builder.compare().lessThanOrEqualOp().left(var).right(max))
            .build()
    }
    
    /// Creates an is-one-of rule.
    ///
    /// This creates a rule that checks if a value is one of a list of options.
    pub fn is_one_of(&self, var_path: &str, options: Vec<impl Into<DataValue<'a>>>) -> Logic<'a> {
        let builder = super::rule_builder(self.arena);
        let var = builder.var(var_path).build();
        
        // Convert options to Logic rules
        let options_array = options.into_iter()
            .map(|opt| builder.value(opt.into()))
            .collect::<Vec<_>>();
        
        let array = builder.array().arrayLiteralOp(options_array);
        
        builder.array().inOp(var, array)
    }
    
    /// Creates a conditional assignment rule.
    ///
    /// This creates a rule that assigns one of two values based on a condition.
    pub fn conditional_value(
        &self, 
        condition: Logic<'a>,
        true_value: impl Into<DataValue<'a>>,
        false_value: impl Into<DataValue<'a>>
    ) -> Logic<'a> {
        let builder = super::rule_builder(self.arena);
        
        builder
            .control()
            .ifOp()
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
        default_value: Option<T>
    ) -> Logic<'a> {
        let builder = super::rule_builder(self.arena);
        
        // Start building a chain of if-then-else
        let mut current_rule = None;
        
        let var = builder.var(var_path).build();
        
        // Build the chain in reverse (from last to first)
        for (key, value) in mapping.iter().rev() {
            let comparison = builder
                .compare()
                .equalOp()
                .left(var.clone())
                .right(builder.value(key.clone()));
                
            if let Some(else_rule) = current_rule {
                current_rule = Some(
                    builder
                        .control()
                        .ifOp()
                        .condition(comparison)
                        .then(builder.value(value.clone()))
                        .else_branch(else_rule)
                        .build()
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
                        .ifOp()
                        .condition(comparison)
                        .then(builder.value(value.clone()))
                        .else_branch(else_value)
                        .build()
                );
            }
        }
        
        current_rule.unwrap_or_else(|| var)
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
        
        let mut concat = builder.string_ops().concatOp();
        
        for (is_var, value) in template_parts {
            if is_var {
                concat = concat.part(builder.var(value).build());
            } else {
                concat = concat.part(builder.string_value(value));
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
        let is_missing = builder.missingOp(builder.string_value(var_path));
        
        // If it's missing, coalesce the rest of the variables
        let rest_coalesce = self.coalesce(vars[1..].to_vec());
        
        builder
            .control()
            .ifOp()
            .condition(is_missing)
            .then(rest_coalesce)
            .else_branch(var)
            .build()
    }
} 