use datalogic_rs::*;
use serde_json::json;

fn main() {
    let logic = json!({
		"reduce": [
			[1, 2, 3, 4],
			{"+": [{"var": "current"}, {"var": "accumulator"}]},
			0
		]
    });
    println!("Logic: {:?}", logic);

    let data = json!({
        "cart": {
            "total": 120.00
        },
        "user": {
            "membership": "premium"
        }
    });

    let rule = Rule::from_value(&logic).unwrap();
    println!("Rule: {:#?}", rule);

    let result = JsonLogic::apply(&rule, &data).unwrap();
    println!("Result: {}", result);
}
