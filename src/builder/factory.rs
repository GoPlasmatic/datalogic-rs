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
            .and()
            .add(builder.compare().greater_than().left(var.clone()).right(min))
            .add(builder.compare().less_than().left(var).right(max))
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
            .and()
            .add(builder.compare().greater_than_or_equal().left(var.clone()).right(min))
            .add(builder.compare().less_than_or_equal().left(var).right(max))
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
        
        let array = builder.array().array_literal(options_array);
        
        builder.array().in_array(var, array)
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
            .if_then()
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
        
        // Process mapping in reverse to build the nested if-then-else
        for (key, value) in mapping.iter().rev() {
            // For each mapping entry, build the components separately 
            let var_node = builder.var(var_path).build();
            let key_node = builder.value(key.clone());
            
            // Create an equality comparison
            let comparison_op = Logic::operator(
                crate::logic::OperatorType::Comparison(crate::ComparisonOp::Equal),
                vec![var_node, key_node],
                self.arena
            );
            
            // Build an if-then-else using the comparison as condition
            let next_rule = builder
                .control()
                .if_then()
                .condition(comparison_op)
                .then(builder.value(value.clone()))
                .else_branch(current_rule.unwrap_or_else(|| {
                    // For the innermost else, use the default or null
                    if let Some(ref default) = default_value {
                        builder.value(default.clone())
                    } else {
                        builder.null()
                    }
                }))
                .build();
                
            current_rule = Some(next_rule);
        }
        
        current_rule.unwrap_or_else(|| {
            // If no mapping was provided, return the default or null
            if let Some(default) = default_value {
                builder.value(default)
            } else {
                builder.null()
            }
        })
    }
    
    /// Creates a string template rule.
    ///
    /// This creates a rule that builds a string from a template,
    /// replacing placeholders with variable values.
    pub fn string_template(&self, template_parts: Vec<(bool, &str)>) -> Logic<'a> {
        let builder = super::rule_builder(self.arena);
        let concat = builder.string_builder().concat();
        
        // Build the concatenation rule
        let mut concat_builder = concat;
        for (is_var, part) in template_parts {
            if is_var {
                // This part is a variable reference
                concat_builder = concat_builder.var(part);
            } else {
                // This part is a literal string
                concat_builder = concat_builder.string(part);
            }
        }
        
        concat_builder.build()
    }
    
    /// Creates a null coalescing rule.
    ///
    /// This creates a rule that returns the first non-null value from a list.
    pub fn coalesce(&self, vars: Vec<&str>) -> Logic<'a> {
        let builder = super::rule_builder(self.arena);
        
        // Create variable references
        let var_refs = vars.into_iter()
            .map(|path| builder.var(path).build())
            .collect::<Vec<_>>();
            
        // Build the operator directly since there's no dedicated builder for it
        Logic::operator(
            crate::logic::OperatorType::Coalesce,
            var_refs,
            self.arena,
        )
    }
} 