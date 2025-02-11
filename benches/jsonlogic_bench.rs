use criterion::{criterion_group, criterion_main, Criterion};
use datalogic_rs::*;
use serde_json::Value;
use std::fs;

lazy_static::lazy_static! {
    static ref TEST_CASES: Vec<(Rule, Value, Value)> = {
        let response = fs::read_to_string("tests/test-cases/legacy.json").unwrap();
       
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

fn bench_apply_all_rules(c: &mut Criterion) {
    let mut group = c.benchmark_group("jsonlogic_rules");
    group.sampling_mode(criterion::SamplingMode::Linear);
    group.sample_size(50);
    
    group.bench_function("apply_all_rules", |b| {
        b.iter(|| {
            for (rule, data, expected) in TEST_CASES.iter() {
                if let Ok(result) = JsonLogic::apply(rule, data) {
                    assert_eq!(result, *expected);
                }
            }
        })
    });
    group.finish();
}

criterion_group!(
    name = benches;
    config = Criterion::default();
    targets = bench_apply_all_rules
);
criterion_main!(benches);