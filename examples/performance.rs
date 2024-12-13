use datalogic_rs::{JsonLogic, Rule};
use serde_json::json;
use std::time::Instant;

fn main() {
    let logic = json!({
        "if": [
            {"missing": ["profile.name", "profile.age"]},
            "Missing required fields",
            {"and": [
                {">=": [{"var": "profile.age"}, 18]},
                {"some": [
                    {"var": "orders"},
                    {"all": [
                        {"var": ""},
                        {"and": [
                            {">": [{"var": "amount"}, 0]},
                            {"in": ["COMPLETED", {"var": "status"}]}
                        ]}
                    ]}
                ]},
                {"merge": [
                    {"map": [
                        {"var": "profile.interests"},
                        {"cat": ["User likes ", {"var": ""}]}
                    ]},
                    [{"substr": [
                        {"var": "profile.bio"},
                        0,
                        {"min": [
                            100,
                            {"+": [
                                {"var": "profile.age"},
                                {"reduce": [
                                    {"var": "orders"},
                                    {"+": [{"var": "accumulator"}, {"var": "current.amount"}]},
                                    0
                                ]}
                            ]}
                        ]}
                    ]}]
                ]},
                {"!": {"none": [
                    {"filter": [
                        {"var": "friends"},
                        {"and": [
                            {"==": [{"var": "status"}, "ACTIVE"]},
                            {">=": [{"var": "connection_strength"}, 0.7]}
                        ]}
                    ]},
                    {"==": [{"var": "premium"}, true]}
                ]}}
            ]}
        ]
    });

    let data = json!({
        "profile": {
            "name": "John Doe",
            "age": 25,
            "bio": "Software developer with passion for coding and innovation",
            "interests": ["coding", "reading", "music"]
        },
        "orders": [
            {"amount": 100, "status": "COMPLETED"},
            {"amount": 50, "status": "COMPLETED"},
            {"amount": 75, "status": "PENDING"}
        ],
        "friends": [
            {"status": "ACTIVE", "premium": true, "connection_strength": 0.9},
            {"status": "ACTIVE", "premium": false, "connection_strength": 0.8},
            {"status": "INACTIVE", "premium": true, "connection_strength": 0.6}
        ]
    });

    // Convert logic to Rule
    let rule = Rule::from_value(&logic).unwrap();

    // Warm-up
    for _ in 0..100 {
        JsonLogic::apply(&rule, &data).unwrap();
    }

    // Measure performance of the new apply_rule function
    let start = Instant::now();
    for _ in 0..100000 {
        JsonLogic::apply(&rule, &data).unwrap();
    }
    let duration_apply_rule = start.elapsed();

    println!("Apply Rule duration: {:?}", duration_apply_rule);
}