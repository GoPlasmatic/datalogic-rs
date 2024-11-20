use datalogic_rs::JsonLogic;
use serde_json::json;

fn main() {
    let logic = JsonLogic::new();

    // Complex discount rule:
    // - 20% off if cart total > $100 AND user is premium member
    // - OR 10% off if cart has more than 5 items
    let discount_rule = json!({
        "or": [
            {"and": [
                {">": [{"var": "cart.total"}, 100]},
                {"==": [{"var": "user.membership"}, "premium"]}
            ]},
            {">": [{"var": "cart.item_count"}, 5]}
        ]
    });

    let customer_data = json!({
        "cart": {
            "total": 120.00,
            "item_count": 3
        },
        "user": {
            "membership": "premium"
        }
    });

    let applies_for_discount = logic.apply(&discount_rule, &customer_data).unwrap();
    assert_eq!(applies_for_discount, json!(true));
}