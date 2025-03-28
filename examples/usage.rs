use datalogic_rs::DataLogic;

fn main() {
    // Basic usage
    basic_example();

    // Step-by-step usage
    step_by_step_example();
}

fn basic_example() {
    println!("Basic example:");

    let dl = DataLogic::new();

    // Parse and evaluate in one step
    let result = dl
        .evaluate_str(
            r#"{ ">": [{"var": "temp"}, 100] }"#,
            r#"{"temp": 110, "name": "user"}"#,
            None,
        )
        .unwrap();

    println!("Is temperature > 100? {}\n", result);
}

fn step_by_step_example() {
    println!("Step-by-step example:");

    let dl = DataLogic::new();

    // 1. Parse the data
    let data = dl.parse_data(r#"{"temp": 110, "name": "user"}"#).unwrap();
    println!("Parsed data: {}", data);

    // 2. Parse the logic rule
    let rule = dl
        .parse_logic(r#"{ ">": [{"var": "temp"}, 100] }"#, None)
        .unwrap();
    println!("Parsed rule: {:?}", rule);

    // 3. Evaluate the rule with the data
    let result = dl.evaluate(&rule, &data).unwrap();
    println!("Rule evaluation result: {}", result);
}
