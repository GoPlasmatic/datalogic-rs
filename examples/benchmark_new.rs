use datalogic_rs::*;
use serde_json::Value;
use std::time::{Instant, Duration};
use std::fs;

lazy_static::lazy_static! {
    static ref TEST_CASES: Vec<(Rule, Value, Value)> = {
        let response = fs::read_to_string("tests/suites/comparison/greaterThan.json").unwrap();
        
        let json_data: Vec<Value> = serde_json::from_str(&response)
            .expect("Failed to parse test cases");
        
        json_data.into_iter()
            .filter_map(|entry| {
                if let Value::Object(test_case) = entry {
                    // let description = test_case.get("description").unwrap();
                    let logic = test_case.get("rule").unwrap();
                    let data = test_case.get("data").unwrap_or(&Value::Null);
                    let expected = test_case.get("result").unwrap_or(&Value::Null);
                    // let error_type = test_case.get("error").unwrap_or(&Value::Null);

                    let rule = Rule::from_value(&logic).ok()?;
                    return Some((
                        rule,
                        data.clone(),
                        expected.clone()
                    ));
                }
                None
            })
            .collect()
    };
}

fn calculate_percentile(mut durations: Vec<Duration>, percentile: f64) -> Duration {
    durations.sort();
    let index = (durations.len() as f64 * percentile / 100.0).ceil() as usize - 1;
    durations[index]
}

fn main() {
    let iterations = 1e5 as u32;
    let mut all_durations: Vec<Duration> = Vec::with_capacity(iterations as usize);

    println!("Running {} iterations for test cases {}", iterations, TEST_CASES.len());
    for _ in 0..iterations {
        let start = Instant::now();
        for (rule, data, _) in TEST_CASES.iter() {
            let _ = JsonLogic::apply(rule, data);
        }
        all_durations.push(start.elapsed());
    }

    // Calculate statistics
    let total_duration: Duration = all_durations.iter().sum();
    let p10 = calculate_percentile(all_durations.clone(), 10.0);
    let avg_duration = total_duration / all_durations.len() as u32;
    let p90 = calculate_percentile(all_durations.clone(), 90.0);
    let p95 = calculate_percentile(all_durations.clone(), 95.0);
    let p99 = calculate_percentile(all_durations, 99.0);
    
    println!("\nBenchmark Results:");
    println!("------------------");
    println!("Total time: {:?}", total_duration);
    println!("10th percentile: {:?}", p10);
    println!("Average time: {:?}", avg_duration);
    println!("90th percentile: {:?}", p90);
    println!("95th percentile: {:?}", p95);
    println!("99th percentile: {:?}", p99);
    println!("Iterations per second: {:.2}", iterations as f64 / total_duration.as_secs_f64());
}