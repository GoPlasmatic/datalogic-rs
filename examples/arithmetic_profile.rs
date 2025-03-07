use datalogic_rs::arena::DataArena;
use datalogic_rs::{evaluate, parse_json};
use datalogic_rs::value::{DataValue, FromJson};
use serde_json::json;
use std::time::Instant;

fn main() {
    // Number of iterations for benchmarking
    let iterations = 1_000_000;
    
    // Create test data
    let arena = DataArena::new();
    let data_json = json!({
        "a": 5,
        "b": 3,
        "c": "10",
        "d": "hello",
        "numbers": [1, 2, 3, 4, 5],
        "strings": ["10", "20", "30", "40", "50"],
        "mixed": [1, "2", 3, "4", 5]
    });
    let data = DataValue::from_json(&data_json, &arena);
    
    // Define test cases
    let test_cases = [
        // Addition
        (json!({"+": [1, 2, 3]}), "add_numbers"),
        (json!({"+": ["10", 20]}), "add_string_number"),
        (json!({"+": [{"var": "a"}, {"var": "b"}]}), "add_vars"),
        (json!({"+": [{"var": "a"}, {"var": "c"}]}), "add_var_string_number"),
        (json!({"+": [{"var": "c"}, {"var": "d"}]}), "add_strings"),
        (json!({"+": [1, 2, 3, 4, 5, 6, 7, 8, 9, 10]}), "add_many_numbers"),
        
        // Subtraction
        (json!({"-": [10, 5]}), "subtract_simple"),
        (json!({"-": [{"var": "a"}, {"var": "b"}]}), "subtract_vars"),
        (json!({"-": [{"var": "a"}, {"var": "c"}]}), "subtract_var_string"),
        
        // Multiplication
        (json!({"*": [2, 3, 4]}), "multiply_simple"),
        (json!({"*": [{"var": "a"}, {"var": "b"}]}), "multiply_vars"),
        (json!({"*": [{"var": "a"}, {"var": "c"}]}), "multiply_var_string"),
        
        // Division
        (json!({"/": [10, 2]}), "divide_simple"),
        (json!({"/": [{"var": "a"}, {"var": "b"}]}), "divide_vars"),
        (json!({"/": [{"var": "a"}, {"var": "c"}]}), "divide_var_string"),
        
        // Modulo
        (json!({"%": [10, 3]}), "modulo_simple"),
        (json!({"%": [{"var": "a"}, {"var": "b"}]}), "modulo_vars"),
        
        // Min/Max
        (json!({"min": [1, 2, 3, 4, 5]}), "min_simple"),
        (json!({"max": [1, 2, 3, 4, 5]}), "max_simple"),
        (json!({"min": [{"var": "numbers"}]}), "min_array"),
        (json!({"max": [{"var": "numbers"}]}), "max_array"),
    ];
    
    println!("Running {} iterations for each test case:", iterations);
    
    // Run benchmarks
    for (rule_json, name) in &test_cases {
        // Parse rule
        let rule_token = parse_json(rule_json, &arena).unwrap();
        
        // Benchmark
        let start = Instant::now();
        
        for _ in 0..iterations {
            let _ = evaluate(rule_token, &data, &arena).unwrap();
        }
        
        let elapsed = start.elapsed();
        let ns_per_op = elapsed.as_nanos() as f64 / iterations as f64;
        let ops_per_sec = 1_000_000_000.0 / ns_per_op;
        
        println!("{}: {:?} ({:.2} ns/op, {:.2} ops/sec)",
                 name, elapsed, ns_per_op, ops_per_sec);
    }
} 