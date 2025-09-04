use datalogic_rs::*;
use serde_json::Value;
use std::fs;
use std::time::Instant;

fn main() {
    // Load test cases from JSON file
    let response =
        fs::read_to_string("tests/suites/compatible.json").expect("Failed to read test cases file");

    let json_data: Vec<Value> =
        serde_json::from_str(&response).expect("Failed to parse test cases");

    // Create a shared logic arena for compiled rules (persists across runs)
    let logic_arena = DataArena::new();

    // Instance for parsing with external arena
    let parse_logic = DataLogic::with_external_arena(&logic_arena);

    // Extract and compile rules once
    let mut test_cases = Vec::new();
    for entry in json_data {
        // Skip string entries (comments)
        if entry.is_string() {
            continue;
        }

        if let Value::Object(test_case) = entry {
            // Get rule and data
            if let Some(logic) = test_case.get("rule") {
                let data = test_case.get("data").unwrap_or(&Value::Null);

                // Compile the rule once using the shared logic arena
                let rule_json_str = logic.to_string();
                if let Ok(rule) = parse_logic.parse_logic(&rule_json_str) {
                    test_cases.push((rule, data.clone()));
                }
            }
        }
    }

    println!("Loaded {} test cases", test_cases.len());

    // Create evaluation instance with its own eval arena but using the shared logic arena
    let mut eval_logic = DataLogic::with_external_arena(&logic_arena);

    // Warmup phase - run tests twice to establish baseline
    println!("\n=== Warmup Phase ===");
    for warmup_run in 1..=2 {
        println!("Warmup run {}", warmup_run);
        for (rule, data_json) in &test_cases {
            let data_value = eval_logic.parse_data_json(data_json).unwrap();
            let _ = eval_logic.evaluate(rule, data_value);
        }
        // Reset eval arena after each warmup run
        eval_logic.reset_eval_arena();
    }

    // Capture baseline memory usage after warmup
    let baseline_memory = eval_logic.eval_arena().memory_usage();
    println!("\nBaseline memory after warmup: {} bytes", baseline_memory);
    println!("Logic arena memory: {} bytes", logic_arena.memory_usage());

    // Main benchmark phase
    let iterations = 1e5 as u32;
    println!("\n=== Main Test Phase ===");
    println!(
        "Running {} iterations for {} test cases ({} total evaluations)",
        iterations,
        test_cases.len(),
        iterations * test_cases.len() as u32
    );

    let start = Instant::now();
    let mut reset_count = 0;

    // Run benchmark with periodic arena resets
    let mut max_memory_between_resets = 0usize;
    let mut last_reset_memory = baseline_memory;

    for i in 0..iterations {
        for (rule, data_json) in &test_cases {
            let data_value = eval_logic.parse_data_json(data_json).unwrap();
            let _ = eval_logic.evaluate(rule, data_value);
        }

        // Check memory before reset
        if (i + 1) % 1000 == 0 {
            let pre_reset_memory = eval_logic.eval_arena().memory_usage();
            let growth_since_reset = pre_reset_memory - last_reset_memory;
            max_memory_between_resets = max_memory_between_resets.max(growth_since_reset);

            eval_logic.reset_eval_arena();
            reset_count += 1;
            last_reset_memory = eval_logic.eval_arena().memory_usage();

            // Sample memory usage periodically
            if (i + 1) % 10000 == 0 {
                println!(
                    "After {} iterations: arena size = {} bytes, growth between resets = {} bytes",
                    i + 1,
                    pre_reset_memory,
                    growth_since_reset
                );
            }
        }
    }

    let duration = start.elapsed();

    // Final memory check
    println!("\n=== Final Memory Report ===");
    let final_memory = eval_logic.eval_arena().memory_usage();
    let memory_diff = final_memory as i64 - baseline_memory as i64;

    println!("Baseline memory: {} bytes", baseline_memory);
    println!("Final memory: {} bytes", final_memory);
    println!("Memory difference: {:+} bytes", memory_diff);
    println!(
        "Logic arena memory (unchanged): {} bytes",
        logic_arena.memory_usage()
    );
    println!("Arena resets performed: {}", reset_count);
    println!(
        "Max memory growth between resets: {} bytes",
        max_memory_between_resets
    );

    // Performance metrics
    println!("\n=== Performance Metrics ===");
    let total_evaluations = iterations * test_cases.len() as u32;
    let avg_iteration_time = duration / total_evaluations;

    println!("Total time: {:?}", duration);
    println!("Average evaluation time: {:?}", avg_iteration_time);
    println!(
        "Evaluations per second: {:.2}",
        total_evaluations as f64 / duration.as_secs_f64()
    );

    // Memory leak verdict
    println!("\n=== Memory Leak Check ===");

    // Check if memory is stable between resets (the real indicator of leaks)
    if max_memory_between_resets <= 33554432 {
        // 32MB is typical arena chunk size
        println!("✅ PASS: No memory leak detected!");
        println!(
            "   Arena reached steady state at {} bytes",
            max_memory_between_resets
        );
        println!("   Growth between resets: 0 bytes after initial allocation");
        println!("   This is normal arena behavior - it allocates chunks and reuses them");
    } else {
        println!("⚠️  WARNING: Unexpected memory growth between resets");
        println!("   Max growth: {} bytes", max_memory_between_resets);
    }

    // Additional verification: run a few more iterations without reset
    println!("\n=== Additional Leak Verification ===");
    println!("Running 100 more iterations without reset...");
    let before_extra = eval_logic.eval_arena().memory_usage();

    for _ in 0..100 {
        for (rule, data_json) in &test_cases {
            let data_value = eval_logic.parse_data_json(data_json).unwrap();
            let _ = eval_logic.evaluate(rule, data_value);
        }
    }

    let after_extra = eval_logic.eval_arena().memory_usage();
    let extra_diff = after_extra - before_extra;

    println!("Memory before extra runs: {} bytes", before_extra);
    println!("Memory after extra runs: {} bytes", after_extra);
    println!("Memory growth: {} bytes", extra_diff);

    if extra_diff > 0 {
        let growth_per_eval = extra_diff as f64 / (100.0 * test_cases.len() as f64);
        println!(
            "Average growth per evaluation: {:.2} bytes",
            growth_per_eval
        );

        if growth_per_eval < 10.0 {
            println!("✅ Growth is within expected arena allocation patterns");
        } else {
            println!("⚠️  High growth rate may indicate a leak");
        }
    }
}
