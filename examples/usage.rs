use datalogic_rs::DataLogic;

fn main() {
    // Basic usage
    basic_example();

    // Step-by-step usage
    step_by_step_example();

    // Datetime example
    datetime_example();
}

fn basic_example() {
    println!("Basic example:");

    let dl = DataLogic::new();

    // Parse and evaluate in one step
    let result = dl
        .evaluate_str(r#"{"datetime": "2022-07-06T13:20:06Z"}"#, r#"{}"#, None)
        .unwrap();

    println!("Datetime: {result}\n");
}

fn step_by_step_example() {
    println!("Step-by-step example:");

    let dl = DataLogic::new();

    // 1. Parse the data
    let data = dl.parse_data(r#"{ "adder": 10 }"#).unwrap();
    println!("Parsed data: {data}");

    // 2. Parse the logic rule
    let rule = dl
        .parse_logic(
            r#"{ "map": [[1,2,3], { "+": [{ "val": [] }, { "val": [[-1], "index"] }] }] }"#,
            None,
        )
        .unwrap();
    println!("Parsed rule: {rule:?}");

    // 3. Evaluate the rule with the data
    let result = dl.evaluate(&rule, &data).unwrap();
    println!("Rule evaluation result: {result}");
}

fn datetime_example() {
    println!("Datetime example:");

    let dl = DataLogic::new();

    let result = dl
        .evaluate_str(
            r#"{
  "cat": [
    { "var": "dates" },
    "T",
    {
      "format_date": [
        { "now": [] },
        "HH:mm:ss"
      ]
    },
    "+00:00"
  ]
}"#,
            r#"{ "dates": "2024-12-31"}"#,
            None,
        )
        .unwrap();
    println!("Datetime: {result}\n");
}
