use criterion::{criterion_group, criterion_main, Criterion};
use datalogic_rs::JsonLogic;
use reqwest::blocking::get;
use serde_json::Value;

lazy_static::lazy_static! {
    static ref TEST_CASES: Vec<(Value, Value, Value)> = {
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
                        return Some((
                            test_case[0].clone(),
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

fn bench_apply_all_rules(c: &mut Criterion) {
    let logic = JsonLogic::new();
    
    c.bench_function("apply_all_rules", |b| {
        b.iter(|| {
            for (rule, data, expected) in TEST_CASES.iter() {
                if let Ok(result) = logic.apply(rule, data) {
                    assert_eq!(result, *expected);
                }
            }
        })
    });
}

criterion_group!(benches, bench_apply_all_rules);
criterion_main!(benches);