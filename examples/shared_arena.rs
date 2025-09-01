//! Example demonstrating shared logic arena for efficient rule reuse.
//!
//! This example shows how to use an external arena to share compiled logic
//! across multiple DataLogic instances, reducing memory usage and compilation overhead.

use datalogic_rs::{DataArena, DataLogic};
use std::collections::HashMap;

/// Example: Single-threaded shared logic
fn single_threaded_example() {
    println!("=== Single-threaded Example ===");

    // Create a shared logic arena
    let logic_arena = DataArena::with_chunk_size(1024 * 1024); // 1MB for logic

    // Parse logic once using a temporary DataLogic
    let rule = {
        let dl = DataLogic::with_external_arena(&logic_arena);
        dl.parse_logic(r#"{"==": [{"var": "x"}, 10]}"#).unwrap()
    };

    // Create multiple DataLogic instances sharing the logic arena
    let mut dl1 = DataLogic::with_external_arena(&logic_arena);
    let mut dl2 = DataLogic::with_external_arena(&logic_arena);

    // Evaluate with different data using different instances
    let data1 = dl1.parse_data(r#"{"x": 10}"#).unwrap();
    let result1 = dl1.evaluate_parsed(rule.root(), data1).unwrap();
    println!("Instance 1 result (x=10): {}", result1);

    let data2 = dl2.parse_data(r#"{"x": 20}"#).unwrap();
    let result2 = dl2.evaluate_parsed(rule.root(), data2).unwrap();
    println!("Instance 2 result (x=20): {}", result2);

    // Reset evaluation arenas without affecting shared logic
    dl1.reset_eval_arena();
    dl2.reset_eval_arena();

    // Rule is still valid after reset
    let data3 = dl1.parse_data(r#"{"x": 10}"#).unwrap();
    let result3 = dl1.evaluate_parsed(rule.root(), data3).unwrap();
    println!("After reset (x=10): {}", result3);
}

/// Example: Multiple instances sharing logic
fn multiple_instances_example() {
    println!("\n=== Multiple Instances Example ===");

    // Create a shared logic arena
    let logic_arena = DataArena::with_chunk_size(1024 * 1024);

    // Pre-compile rules
    let temp_dl = DataLogic::with_external_arena(&logic_arena);
    let rule1 = temp_dl
        .parse_logic(r#"{">=": [{"var": "age"}, 18]}"#)
        .unwrap();
    let rule2 = temp_dl
        .parse_logic(r#"{"==": [{"var": "status"}, "active"]}"#)
        .unwrap();

    // Create multiple instances that will share the compiled rules
    let instances: Vec<_> = (0..4)
        .map(|_| DataLogic::with_external_arena(&logic_arena))
        .collect();

    // Use different instances to evaluate the same rules with different data
    for (i, dl) in instances.iter().enumerate() {
        // Use pre-compiled rules
        let rule = if i % 2 == 0 { &rule1 } else { &rule2 };

        // Evaluate with instance-specific data
        let data = format!(r#"{{"age": {}, "status": "active"}}"#, 15 + i * 5);
        let parsed_data = dl.parse_data(&data).unwrap();
        let result = dl.evaluate(rule, parsed_data).unwrap();

        println!("Instance {}: {}", i, result);
    }

    println!("\nNote: DataArena is not thread-safe due to bumpalo's internal Cell usage.");
    println!("For true multi-threading, each thread would need its own DataLogic with");
    println!("its own arena, and rules would need to be compiled separately per thread.");
}

/// Example: Rule cache with external arena
#[derive(Debug)]
struct RuleCache<'a> {
    logic_arena: &'a DataArena,
    compiled_rules: HashMap<String, datalogic_rs::Logic<'a>>,
}

impl<'a> RuleCache<'a> {
    fn new(arena: &'a DataArena) -> Self {
        Self {
            logic_arena: arena,
            compiled_rules: HashMap::new(),
        }
    }

    fn compile(&mut self, name: &str, rule: &str) -> Result<(), datalogic_rs::LogicError> {
        if self.compiled_rules.contains_key(name) {
            return Ok(());
        }

        // Use temporary DataLogic for parsing
        let dl = DataLogic::with_external_arena(self.logic_arena);
        let logic = dl.parse_logic(rule)?;
        self.compiled_rules.insert(name.to_string(), logic);
        Ok(())
    }

    fn get(&self, name: &str) -> Option<&datalogic_rs::Logic<'a>> {
        self.compiled_rules.get(name)
    }
}

fn rule_cache_example() {
    println!("\n=== Rule Cache Example ===");

    // Create shared arena for all rules
    let logic_arena = DataArena::with_chunk_size(10 * 1024 * 1024); // 10MB

    // Create rule cache
    let mut cache = RuleCache::new(&logic_arena);

    // Compile rules once
    cache
        .compile("adult", r#"{">=": [{"var": "age"}, 18]}"#)
        .unwrap();
    cache
        .compile("senior", r#"{">=": [{"var": "age"}, 65]}"#)
        .unwrap();
    cache
        .compile(
            "teen",
            r#"{"and": [{">=": [{"var": "age"}, 13]}, {"<": [{"var": "age"}, 20]}]}"#,
        )
        .unwrap();

    // Create evaluator instance
    let mut dl = DataLogic::with_external_arena(&logic_arena);

    // Evaluate using cached rules
    for age in [10, 16, 25, 70] {
        println!("\nAge {}: ", age);
        let data = dl.parse_data(&format!(r#"{{"age": {}}}"#, age)).unwrap();

        let is_adult = dl.evaluate(cache.get("adult").unwrap(), data).unwrap();
        println!("  Adult: {}", is_adult);

        let is_senior = dl.evaluate(cache.get("senior").unwrap(), data).unwrap();
        println!("  Senior: {}", is_senior);

        let is_teen = dl.evaluate(cache.get("teen").unwrap(), data).unwrap();
        println!("  Teen: {}", is_teen);

        dl.reset_eval_arena(); // Clear eval data, keep logic
    }
}

/// Example: Batch processing with shared rules
fn batch_processing_example() {
    println!("\n=== Batch Processing Example ===");

    // Create shared arena
    let logic_arena = DataArena::new();

    // Compile complex validation rule once
    let validation_rule = {
        let dl = DataLogic::with_external_arena(&logic_arena);
        dl.parse_logic(
            r#"{
            "and": [
                {">=": [{"var": "score"}, 0]},
                {"<=": [{"var": "score"}, 100]},
                {"!=": [{"var": "status"}, "invalid"]}
            ]
        }"#,
        )
        .unwrap()
    };

    // Process batch of data items
    let data_items = [
        r#"{"score": 85, "status": "active"}"#,
        r#"{"score": 120, "status": "active"}"#,
        r#"{"score": 50, "status": "invalid"}"#,
        r#"{"score": 95, "status": "pending"}"#,
    ];

    let mut dl = DataLogic::with_external_arena(&logic_arena);

    for (i, data_str) in data_items.iter().enumerate() {
        let data = dl.parse_data(data_str).unwrap();
        let result = dl.evaluate(&validation_rule, data).unwrap();
        println!("Item {}: {} -> Valid: {}", i, data_str, result);

        // Clear evaluation arena after each item to prevent memory growth
        dl.reset_eval_arena();
    }
}

fn main() {
    single_threaded_example();
    multiple_instances_example();
    rule_cache_example();
    batch_processing_example();

    println!("\n=== All examples completed successfully! ===");
}
