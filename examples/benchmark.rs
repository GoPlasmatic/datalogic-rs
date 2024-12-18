use datalogic_rs::*;
use reqwest::blocking::get;
use serde_json::Value;
use std::time::Instant;

lazy_static::lazy_static! {
    static ref TEST_CASES: Vec<(Rule, Value, Value)> = {
        let response = get("http://jsonlogic.com/tests.json")
            .expect("Failed to fetch test cases")
            .text()
            .expect("Failed to read response");
        
        let json_data: Vec<Value> = serde_json::from_str(&response)
            .expect("Failed to parse test cases");
        
        json_data.into_iter()
            .filter_map(|entry| {
                if let Value::Array(test_case) = entry {
                    if test_case.len() == 3 {
                        let rule = Rule::from_value(&test_case[0]).ok()?;
                        return Some((
                            rule,
                            test_case[1].clone(),
                            test_case[2].clone()
                        ));
                    }
                }
                None
            })
            .collect()
    };
}

fn main() {
    let iterations = 1e5 as u32;
    
    let start = Instant::now();
    
    for _ in 0..iterations {
        for (rule, data, expected) in TEST_CASES.iter() {
            if let Ok(result) = JsonLogic::apply(rule, data) {
                assert_eq!(result, *expected);
            }
        }
    }
    let duration = start.elapsed();
    let avg_iteration_time = duration / iterations as u32;
    
    println!("Total time: {:?}", duration);
    println!("Average iteration time: {:?}", avg_iteration_time);
    println!("Iterations per second: {:.2}", iterations as f64 / duration.as_secs_f64());
}