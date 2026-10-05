//! `Roots` and `Engine::truthy_of` in a build without `serde_json`: owned
//! and parsed parts only.

use datalogic_rs::datavalue::OwnedDataValue;
use datalogic_rs::{Engine, ParsedData, Roots};

#[test]
fn owned_and_parsed_roots() {
    let data = OwnedDataValue::from_json(r#"{"n": 40}"#).unwrap();
    let metadata = ParsedData::from_json(r#"{"step": 2}"#).unwrap();
    let roots = Roots::new().root("data", &data).root("metadata", &metadata);

    let engine = Engine::new();
    let rule = r#"{"+": [{"var": "data.n"}, {"var": "metadata.step"}]}"#;
    assert_eq!(engine.eval_str(rule, &roots).unwrap(), "42");

    let logic = engine.compile(rule).unwrap();
    let mut session = engine.session();
    assert_eq!(session.eval_str(&logic, &roots).unwrap(), "42");
}

#[test]
fn truthy_of_owned_and_parsed() {
    let engine = Engine::new();
    for (json, expected) in [
        ("{}", false),
        (r#"{"a": 1}"#, true),
        ("[]", false),
        ("0", false),
        ("\"x\"", true),
    ] {
        let owned = OwnedDataValue::from_json(json).unwrap();
        let parsed = ParsedData::from_json(json).unwrap();
        assert_eq!(engine.truthy_of(&owned), expected, "{json}");
        assert_eq!(engine.truthy_of(&parsed), expected, "{json}");
    }
}
