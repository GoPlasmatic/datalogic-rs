use datalogic_rs::JsonLogic;
use serde_json::json;

fn main() {
    let logic = JsonLogic::new();

    let rule = json!({
        "filter": [
            {
                "var": "locales"
            },
            {
                "!==": [
                    {
                        "var": "code"
                    },
                    {
                        "var": "../../locale"
                    }
                ]
            }
        ]
    });
    let data = json!({
        "locale": "pl",
        "locales": [
            {
                "name": "Israel",
                "code": "he",
                "flag": "🇮🇱",
                "iso": "he-IL",
                "dir": "rtl"
            },
            {
                "name": "українська",
                "code": "ue",
                "flag": "🇺🇦",
                "iso": "uk-UA",
                "dir": "ltr"
            },
            {
                "name": "Polski",
                "code": "pl",
                "flag": "🇵🇱",
                "iso": "pl-PL",
                "dir": "ltr"
            }
        ]
    });
    
    let result = logic.apply(&rule, &data).unwrap();
    println!("Result: {}", result);
}