use datalogic_rs::value::json_to_data_value;
use datalogic_rs::{evaluate, DataArena, IntoLogic};
use serde_json::json;
use std::time::Instant;

fn main() {
    // Number of iterations for the benchmark
    let iterations = 100_000; // Reduced iterations to avoid memory issues

    // Create a data arena for memory management
    let arena = DataArena::new();

    // Create test data
    let data = json!({
        "numbers": [1, 2, 3, 4, 5, 6, 7, 8, 9, 10],
        "strings": ["hello", "world", "benchmark", "test"],
        "nested": {
            "values": [10, 20, 30, 40, 50],
            "flag": true
        }
    });

    // Convert to DataValue
    let data_value = json_to_data_value(&data, &arena);

    // Create a simple map operation
    let map_logic = json!({
        "map": [
            {"var": "numbers"},
            {"*": [{"var": ""}, 2]}
        ]
    });

    // Create a more complex operation (filter + map + reduce)
    let complex_logic = json!({
        "reduce": [
            {"map": [
                {"filter": [
                    {"var": "numbers"},
                    {">": [{"var": ""}, 5]}
                ]},
                {"*": [{"var": ""}, 3]}
            ]},
            {"+": [{"var": "current"}, {"var": "accumulator"}]},
            0
        ]
    });

    // Parse the logic
    let map_parsed = map_logic.to_logic(&arena).unwrap();
    let complex_parsed = complex_logic.to_logic(&arena).unwrap();

    // Benchmark the map operation
    println!(
        "Benchmarking simple map operation ({} iterations):",
        iterations
    );
    let start = Instant::now();

    // Create a temporary arena for evaluations
    let mut temp_arena = DataArena::new();

    for _ in 0..iterations {
        let _ = evaluate(map_parsed.root(), &data_value, &temp_arena).unwrap();
        temp_arena.reset();
    }

    let duration = start.elapsed();
    let total_ms =
        duration.as_secs() as f64 * 1000.0 + duration.subsec_nanos() as f64 / 1_000_000.0;
    let avg_ns = (duration.as_secs() as f64 * 1_000_000_000.0 + duration.subsec_nanos() as f64)
        / iterations as f64;
    let ops_per_sec = iterations as f64
        / (duration.as_secs() as f64 + duration.subsec_nanos() as f64 / 1_000_000_000.0);

    println!("  Total time: {:.2}ms", total_ms);
    println!("  Average iteration time: {:.2}ns", avg_ns);
    println!("  Iterations per second: {:.2}", ops_per_sec);

    // Benchmark the complex operation
    println!(
        "\nBenchmarking complex operation (filter + map + reduce) ({} iterations):",
        iterations
    );
    let start = Instant::now();

    // Reset the temporary arena
    temp_arena.reset();

    for _ in 0..iterations {
        let _ = evaluate(complex_parsed.root(), &data_value, &temp_arena).unwrap();
        temp_arena.reset();
    }

    let duration = start.elapsed();
    let total_ms =
        duration.as_secs() as f64 * 1000.0 + duration.subsec_nanos() as f64 / 1_000_000.0;
    let avg_ns = (duration.as_secs() as f64 * 1_000_000_000.0 + duration.subsec_nanos() as f64)
        / iterations as f64;
    let ops_per_sec = iterations as f64
        / (duration.as_secs() as f64 + duration.subsec_nanos() as f64 / 1_000_000_000.0);

    println!("  Total time: {:.2}ms", total_ms);
    println!("  Average iteration time: {:.2}ns", avg_ns);
    println!("  Iterations per second: {:.2}", ops_per_sec);
}
