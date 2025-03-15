use datalogic_rs::*;
use serde_json::json;
use std::time::Instant;

fn main() {
    let iterations = 5000; // Reduced iterations but with larger arrays
    println!("Running array benchmarks with {} iterations", iterations);
    
    let logic_arena = DataArena::new();
    
    println!("\nSmall Array Tests (20 elements):");
    benchmark_reduce_sum(&logic_arena, iterations, 20);
    benchmark_map(&logic_arena, iterations, 20);
    benchmark_filter(&logic_arena, iterations, 20);
    benchmark_complex(&logic_arena, iterations, 20);
    
    println!("\nLarge Array Tests (1000 elements):");
    benchmark_reduce_sum(&logic_arena, iterations / 10, 1000);
    benchmark_map(&logic_arena, iterations / 10, 1000);
    benchmark_filter(&logic_arena, iterations / 10, 1000);
    benchmark_complex(&logic_arena, iterations / 10, 1000);
}

fn benchmark_reduce_sum(logic_arena: &DataArena, iterations: u32, array_size: usize) {
    // Create an array of the specified size
    let mut array_values = Vec::with_capacity(array_size);
    for i in 1..=array_size {
        array_values.push(i);
    }
    let array_json = json!(array_values);
    let data_json = json!({"array": array_json});
    
    // Create reduce operation: {"reduce": [{"var": "array"}, "+", 0]}
    let reduce_json = json!({"reduce": [{"var": "array"}, "+", 0]});
    
    // Parse logic
    let reduce_logic = reduce_json.to_logic(logic_arena).unwrap();
    let data_value = DataValue::from_json(&data_json, logic_arena);
    
    // Create a fresh arena for each benchmark
    let mut eval_arena = DataArena::new();
    
    // Run benchmark
    let start = Instant::now();
    for _ in 0..iterations {
        let _ = evaluate(reduce_logic.root(), &data_value, &eval_arena);
        eval_arena.reset();
    }
    let duration = start.elapsed();
    
    println!("Reduce + sum operation (array size: {}):", array_size);
    println!("  Total time: {:?}", duration);
    println!("  Average iteration time: {:?}", duration / iterations);
    println!("  Iterations per second: {:.2}", iterations as f64 / duration.as_secs_f64());
}

fn benchmark_map(logic_arena: &DataArena, iterations: u32, array_size: usize) {
    // Create an array of the specified size
    let mut array_values = Vec::with_capacity(array_size);
    for i in 1..=array_size {
        array_values.push(i);
    }
    let array_json = json!(array_values);
    let data_json = json!({"array": array_json});
    
    // Create map operation: {"map": [{"var": "array"}, {"*": [{"var":""}, 2]}]}
    let map_json = json!({"map": [{"var": "array"}, {"*": [{"var":""}, 2]}]});
    
    // Parse logic
    let map_logic = map_json.to_logic(logic_arena).unwrap();
    let data_value = DataValue::from_json(&data_json, logic_arena);
    
    // Create a fresh arena for each benchmark
    let mut eval_arena = DataArena::new();
    
    // Run benchmark
    let start = Instant::now();
    for _ in 0..iterations {
        let _ = evaluate(map_logic.root(), &data_value, &eval_arena);
        eval_arena.reset();
    }
    let duration = start.elapsed();
    
    println!("Map operation (array size: {}):", array_size);
    println!("  Total time: {:?}", duration);
    println!("  Average iteration time: {:?}", duration / iterations);
    println!("  Iterations per second: {:.2}", iterations as f64 / duration.as_secs_f64());
}

fn benchmark_filter(logic_arena: &DataArena, iterations: u32, array_size: usize) {
    // Create an array of the specified size
    let mut array_values = Vec::with_capacity(array_size);
    for i in 1..=array_size {
        array_values.push(i);
    }
    let array_json = json!(array_values);
    let data_json = json!({"array": array_json});
    
    // Create filter operation: {"filter": [{"var": "array"}, {">": [{"var":""}, threshold]}]}
    // Select approximately half the elements
    let threshold = array_size / 2;
    let filter_json = json!({"filter": [{"var": "array"}, {">": [{"var":""}, threshold]}]});
    
    // Parse logic
    let filter_logic = filter_json.to_logic(logic_arena).unwrap();
    let data_value = DataValue::from_json(&data_json, logic_arena);
    
    // Create a fresh arena for each benchmark
    let mut eval_arena = DataArena::new();
    
    // Run benchmark
    let start = Instant::now();
    for _ in 0..iterations {
        let _ = evaluate(filter_logic.root(), &data_value, &eval_arena);
        eval_arena.reset();
    }
    let duration = start.elapsed();
    
    println!("Filter operation (array size: {}):", array_size);
    println!("  Total time: {:?}", duration);
    println!("  Average iteration time: {:?}", duration / iterations);
    println!("  Iterations per second: {:.2}", iterations as f64 / duration.as_secs_f64());
}

fn benchmark_complex(logic_arena: &DataArena, iterations: u32, array_size: usize) {
    // Create an array of the specified size
    let mut array_values = Vec::with_capacity(array_size);
    for i in 1..=array_size {
        array_values.push(i);
    }
    let array_json = json!(array_values);
    let threshold = array_size / 2;
    let data_json = json!({
        "array": array_json,
        "threshold": threshold
    });
    
    // Create a complex operation that combines filter, map, and reduce:
    // First filter values > threshold, then double each one, then sum them
    let complex_json = json!({
        "reduce": [
            {"map": [
                {"filter": [
                    {"var": "array"},
                    {">": [{"var": ""}, {"var": "threshold"}]}
                ]},
                {"*": [{"var": ""}, 2]}
            ]},
            "+",
            0
        ]
    });
    
    // Parse logic
    let complex_logic = complex_json.to_logic(logic_arena).unwrap();
    let data_value = DataValue::from_json(&data_json, logic_arena);
    
    // Create a fresh arena for benchmark
    let mut eval_arena = DataArena::new();
    
    // Run benchmark
    let start = Instant::now();
    for _ in 0..iterations {
        let _ = evaluate(complex_logic.root(), &data_value, &eval_arena);
        eval_arena.reset();
    }
    let duration = start.elapsed();
    
    println!("Complex operation (filter > map > reduce) (array size: {}):", array_size);
    println!("  Total time: {:?}", duration);
    println!("  Average iteration time: {:?}", duration / iterations);
    println!("  Iterations per second: {:.2}", iterations as f64 / duration.as_secs_f64());
} 