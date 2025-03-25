use datalogic_rs::{DataLogic, Result};

fn main() -> Result<()> {
    // Basic usage
    basic_example()?;
    
    // Step-by-step usage
    step_by_step_example()?;
    
    Ok(())
}

fn basic_example() -> Result<()> {
    println!("Basic example:");
    
    let dl = DataLogic::new();
    
    // Parse and evaluate in one step
    let result = dl.evaluate_str(
        r#"{ ">": [{"var": "temp"}, 100] }"#,
        r#"{"temp": 110, "name": "user"}"#,
        None
    )?;
    
    println!("Is temperature > 100? {}\n", result);
    
    Ok(())
}

fn step_by_step_example() -> Result<()> {
    println!("Step-by-step example:");
    
    let dl = DataLogic::new();
    
    // 1. Parse the data
    let data = dl.parse_data(r#"{"temp": 110, "name": "user"}"#)?;
    println!("Parsed data: {}", data);
    
    // 2. Parse the logic rule
    let rule = dl.parse_logic(r#"{ ">": [{"var": "temp"}, 100] }"#, None)?;
    
    // 3. Evaluate the rule with the data
    let result = dl.evaluate(&rule, &data)?;
    println!("Rule evaluation result: {}", result);
    
    Ok(())
}
