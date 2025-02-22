use criterion::{black_box, criterion_group, criterion_main, Criterion};
use datalogic_rs::{JsonLogic, Rule};
use serde_json::json;

/// Creates a complex business rule for benchmarking that simulates a real-world
/// dynamic pricing system with customer eligibility and tiered discounts.
/// 
/// # Rule Logic
/// 
/// ## Eligibility Criteria (ALL must be met):
/// 1. Customer age > 21
/// 2. Has at least one purchase > $100
/// 3. Not blacklisted
/// 4. Premium tier customer
/// 
/// ## Discount Calculation:
/// For each purchase amount:
/// - 15% discount (0.85) if purchase >= $500
/// - 10% discount (0.90) if purchase >= $300
/// - 5% discount (0.95) for all other amounts
/// 
/// ## Rule Behavior:
/// - If customer is eligible: Apply tiered discounts to all purchases
/// - If not eligible: Return original purchase amounts unchanged
/// 
/// # Example
/// ```json
/// Input:
/// {
///   "customer": {
///     "age": 25,
///     "tier": "premium",
///     "blacklisted": false,
///     "purchases": [150.0, 350.0, 550.0]
///   }
/// }
/// 
/// Output:
/// [142.5, 315.0, 467.5]  // 5%, 10%, 15% discounts applied
/// ```
fn create_complex_rule() -> Rule {
    // This rule combines multiple operators to test real-world scenarios
    Rule::from_value(&json!({
        "if": [
            // Eligibility check using AND operator
            {"and": [
                {">": [{"var": "customer.age"}, 21]},          // Age verification
                {"some": [                                     // Purchase history check
                    {"var": "customer.purchases"},
                    {">": [{"var": ""}, 100]}                 // At least one purchase > $100
                ]},
                {"!": {"var": "customer.blacklisted"}},       // Not blacklisted
                {"in": ["premium", {"var": "customer.tier"}]}  // Premium tier check
            ]},
            // Discount calculation using map operator
            {
                "map": [
                    {"var": "customer.purchases"},            // Process each purchase
                    {
                        "*": [
                            {"var": ""},                      // Current purchase amount
                            {
                                "if": [
                                    {">=": [{"var": ""}, 500]},
                                    0.85,  // 15% discount for purchases >= $500
                                    {
                                        "if": [
                                            {">=": [{"var": ""}, 300]},
                                            0.9,   // 10% discount for purchases >= $300
                                            0.95   // 5% discount for all other purchases
                                        ]
                                    }
                                ]
                            }
                        ]
                    }
                ]
            },
            // Fallback: preserve original purchase amounts if not eligible
            {"preserve": [{"var": "customer.purchases"}]}
        ]
    })).unwrap()
}

fn generate_test_data(size: usize) -> Vec<serde_json::Value> {
    let mut data = Vec::with_capacity(size);
    
    // Generate various test cases
    for i in 0..size {
        let purchases = (0..5).map(|j| {
            100.0 + (i as f64 * 100.0) + (j as f64 * 50.0)
        }).collect::<Vec<_>>();

        data.push(json!({
            "customer": {
                "age": 25 + (i % 20),
                "tier": if i % 3 == 0 { "premium" } else { "standard" },
                "blacklisted": i % 10 == 0,
                "purchases": purchases
            }
        }));
    }
    
    data
}

fn benchmark_complex_rule(c: &mut Criterion) {
    let rule = create_complex_rule();
    let test_data = generate_test_data(100);
    
    c.bench_function("complex_rule_evaluation", |b| {
        b.iter(|| {
            for data in test_data.iter() {
                black_box(JsonLogic::apply(&rule, black_box(data)).unwrap());
            }
        })
    });
}

criterion_group!(benches, benchmark_complex_rule);
criterion_main!(benches);