use datalogic_rs::*;
use serde_json::Value;
use std::time::Instant;
use std::fs;

lazy_static::lazy_static! {
    static ref TEST_CASES: Vec<(Rule, Value, Value)> = {
        let response = fs::read_to_string("tests/suites/compatible.json").unwrap();
        
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

fn main() {
    let iterations = 1e5 as u32;
    let start = Instant::now();
    
    println!("Running {} iterations for test cases {}", iterations, TEST_CASES.len());
    for (rule, data, _) in TEST_CASES.iter() {
        for _ in 0..iterations {
            let _ = JsonLogic::apply(rule, data);
        }
    }
    let duration = start.elapsed();
    let avg_iteration_time = duration / iterations as u32;
    
    println!("Total time: {:?}", duration);
    println!("Average iteration time: {:?}", avg_iteration_time);
    println!("Iterations per second: {:.2}", iterations as f64 / duration.as_secs_f64());
}