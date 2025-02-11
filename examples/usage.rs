use datalogic_rs::*;
use serde_json::json;

fn main() {
	let logic = json!({
        "if": [
            {"and": [
                {">": [{"var": "cart.total"}, 100]},
                {"==": [{"var": "user.membership"}, "premium"]}
            ]},
            {"*": [{"var": "cart.total"}, 0.75]}, // 25% discount
            {"*": [{"var": "cart.total"}, 1.0]}   // no discount
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