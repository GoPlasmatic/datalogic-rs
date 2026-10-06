//! `Logic::to_json` must produce JSON that compiles back to a rule that
//! evaluates the same, and `Error::resolve_path` must name the same
//! pointers a traced compile records.
//!
//! Two groups:
//!
//! - Escaping and `val`: rules whose strings hold quotes, backslashes and
//!   control characters, and `val` keys holding a `.`, which used to come
//!   back as invalid JSON or as a `var` path that reads something else.
//! - Differential properties: `compile(to_json(compile(r)))` evaluates as
//!   `r` does, and the pointers `resolve_node_ids` rebuilds for a plain
//!   compile match the ones a traced compile records.
#![cfg(all(feature = "serde_json", feature = "trace"))]

use datalogic_rs::{CustomOperator, DataValue, Engine, Logic, operator::EvalContext};
use proptest::prelude::*;
use serde_json::{Value, json};

/// Returns its first argument.
struct First;

impl CustomOperator for First {
    fn evaluate<'a>(
        &self,
        args: &[&'a DataValue<'a>],
        _ctx: &mut EvalContext<'_, 'a>,
        _arena: &'a datalogic_rs::bumpalo::Bump,
    ) -> datalogic_rs::Result<&'a DataValue<'a>> {
        Ok(args.first().copied().unwrap_or(&DataValue::Null))
    }
}

/// Evaluate `rule` against `data`, as text for comparison.
fn run(engine: &Engine, rule: &str, data: &str) -> String {
    match engine.eval_str(rule, data) {
        Ok(v) => format!("ok {v}"),
        Err(e) => format!("err {:?}", e.code()),
    }
}

/// Compile `rule`, serialize it, check the text is JSON, and check the
/// serialized rule evaluates as the original does against `data`.
fn assert_round_trip(engine: &Engine, rule: &str, data: &str) -> String {
    let compiled = engine.compile(rule).unwrap();
    let back = compiled.to_json();
    serde_json::from_str::<Value>(&back).unwrap_or_else(|e| panic!("{rule} -> {back}: {e}"));
    assert_eq!(
        run(engine, &back, data),
        run(engine, rule, data),
        "{rule} -> {back}"
    );
    back
}

#[test]
fn strings_are_escaped() {
    let engine = Engine::builder()
        .with_constant_folding(false)
        .add_operator("my\"op", First)
        .build();
    let data = json!({"a\"b": 1, "c\\d": 2, "e\nf": 3, "g\u{1}h": 4}).to_string();
    for rule in [
        json!({"var": "a\"b"}),
        json!({"var": ["c\\d", "x\"y"]}),
        json!({"val": "e\nf"}),
        json!({"val": ["g\u{1}h"]}),
        json!({"missing": ["a\"b", "q\"r"]}),
        json!({"missing_some": [1, ["a\"b", "q\"r"]]}),
        json!({"my\"op": [{"var": "a\"b"}]}),
        json!({"cat": ["\"", {"var": "c\\d"}]}),
    ] {
        assert_round_trip(&engine, &rule.to_string(), &data);
    }
}

#[cfg(feature = "ext-control")]
#[test]
fn exists_paths_are_escaped() {
    let engine = Engine::new();
    let data = json!({"a\"b": {"c\\d": 1}}).to_string();
    for rule in [
        json!({"exists": "a\"b"}),
        json!({"exists": ["a\"b", "c\\d"]}),
    ] {
        assert_round_trip(&engine, &rule.to_string(), &data);
    }
}

#[cfg(feature = "error-handling")]
#[test]
fn throw_types_are_escaped() {
    let engine = Engine::new();
    let back = assert_round_trip(&engine, &json!({"throw": "bad\"type"}).to_string(), "{}");
    assert_eq!(back, r#"{"throw": "bad\"type"}"#);
}

#[cfg(feature = "templating")]
#[test]
fn template_keys_are_escaped() {
    let engine = Engine::builder().with_templating(true).build();
    let rule = json!({"a\"b": {"var": "x"}, "c\\d": 1}).to_string();
    assert_round_trip(&engine, &rule, r#"{"x": 5}"#);
}

/// A `val` key holding a `.` reads that one key; written back as a `var`
/// path it would read `a` then `b`.
#[test]
fn dotted_val_keys_stay_val() {
    let engine = Engine::new();
    let data = r#"{"a.b": 1, "a": {"b": 2, "c.d": 3}}"#;
    for (rule, want) in [
        (r#"{"val": "a.b"}"#, r#"{"val": "a.b"}"#),
        (r#"{"val": ["a.b"]}"#, r#"{"val": "a.b"}"#),
        (r#"{"val": ["a", "c.d"]}"#, r#"{"val": ["a", "c.d"]}"#),
        (r#"{"var": "a.b"}"#, r#"{"var": "a.b"}"#),
        (r#"{"val": ["a", "b"]}"#, r#"{"var": "a.b"}"#),
    ] {
        assert_eq!(assert_round_trip(&engine, rule, data), want, "{rule}");
    }
}

#[test]
fn level_markers_round_trip() {
    let engine = Engine::new();
    let rule = r#"{"map": [{"var": "xs"}, {"val": [[2], "k.j"]}]}"#;
    assert_round_trip(&engine, rule, r#"{"xs": [1, 2], "k.j": 7}"#);
}

// ---------------------------------------------------------------------------
// Properties
// ---------------------------------------------------------------------------

/// Path keys, including the characters JSON has to escape and `.`.
fn arb_key() -> impl Strategy<Value = String> {
    prop_oneof![
        3 => "[a-c]{1,2}",
        1 => Just("a.b".to_owned()),
        1 => Just("q\"r".to_owned()),
        1 => Just("s\\t".to_owned()),
        1 => Just("u\nv".to_owned()),
        1 => Just("0".to_owned()),
        1 => any::<String>(),
    ]
}

/// Data shaped so generated paths sometimes hit.
fn arb_data() -> impl Strategy<Value = Value> {
    let leaf = prop_oneof![
        Just(Value::Null),
        any::<bool>().prop_map(Value::Bool),
        (-5i64..5).prop_map(Value::from),
        arb_key().prop_map(Value::String),
    ];
    leaf.prop_recursive(3, 24, 4, |inner| {
        prop_oneof![
            prop::collection::vec(inner.clone(), 0..=3).prop_map(Value::Array),
            prop::collection::btree_map(arb_key(), inner, 0..=4)
                .prop_map(|m| Value::Object(m.into_iter().collect())),
        ]
    })
}

/// Rules over a grammar the serializer must handle: reads in every form,
/// `missing`, string and number literals, and arithmetic, comparison and
/// control operators with their arguments always listed. Literal objects
/// are left out: JSONLogic has no way to write one outside templating.
fn arb_rule() -> impl Strategy<Value = Value> {
    let leaf = prop_oneof![
        Just(Value::Null),
        any::<bool>().prop_map(Value::Bool),
        (-5i64..5).prop_map(Value::from),
        arb_key().prop_map(Value::String),
        arb_key().prop_map(|k| json!({"var": k})),
        arb_key().prop_map(|k| json!({"val": k})),
        prop::collection::vec(arb_key(), 0..=3).prop_map(|ks| json!({"val": ks})),
        (1u32..3, arb_key()).prop_map(|(n, k)| json!({"val": [[n], k]})),
        prop::collection::vec(arb_key(), 1..=3).prop_map(|ks| json!({"missing": ks})),
        (0u32..3, prop::collection::vec(arb_key(), 0..=3))
            .prop_map(|(n, ks)| json!({"missing_some": [n, ks]})),
    ];
    leaf.prop_recursive(4, 32, 3, |inner| {
        let op = prop_oneof![
            Just("+"),
            Just("cat"),
            Just("=="),
            Just("<"),
            Just("if"),
            Just("and"),
            Just("or"),
            Just("!"),
            Just("max"),
            Just("merge"),
        ];
        prop_oneof![
            (op, prop::collection::vec(inner.clone(), 0..=3))
                .prop_map(|(op, args)| json!({ op: args })),
            (arb_key(), inner.clone()).prop_map(|(k, d)| json!({"var": [k, d]})),
            prop::collection::vec(inner.clone(), 0..=3).prop_map(Value::Array),
            (inner.clone(), inner).prop_map(|(xs, body)| json!({"map": [xs, body]})),
        ]
    })
}

/// `rule` with every operator's lone argument wrapped in an array.
fn listed(rule: Value) -> Value {
    match rule {
        Value::Object(map) if map.len() == 1 => {
            let (op, args) = map.into_iter().next().unwrap();
            let args = match args {
                Value::Array(items) => Value::Array(items.into_iter().map(listed).collect()),
                lone => Value::Array(vec![listed(lone)]),
            };
            json!({ op: args })
        }
        Value::Array(items) => Value::Array(items.into_iter().map(listed).collect()),
        other => other,
    }
}

/// Compile `rule`, or `None` when it does not compile.
fn compile(engine: &Engine, rule: &Value) -> Option<Logic> {
    engine.compile(rule).ok()
}

proptest! {
    /// Any string written into a read, a `missing` path or a literal
    /// comes back as valid JSON that reads the same key.
    #[test]
    fn arbitrary_strings_round_trip(key in any::<String>(), value in any::<String>()) {
        let engine = Engine::new();
        let data = json!({ key.clone(): value.clone() }).to_string();
        for rule in [
            json!({"var": key}),
            json!({"val": key}),
            json!({"val": [key]}),
            json!({"var": [key, value]}),
            json!({"missing": [key]}),
            json!({"cat": [value, {"val": key}]}),
        ] {
            let compiled = engine.compile(&rule).unwrap();
            let back = compiled.to_json();
            prop_assert!(serde_json::from_str::<Value>(&back).is_ok(), "{rule} -> {back}");
            prop_assert_eq!(
                run(&engine, &back, &data),
                run(&engine, &rule.to_string(), &data),
                "{} -> {}", rule, back
            );
        }
    }

    /// `compile(to_json(compile(r)))` evaluates as `r` does, with and
    /// without folding.
    #[test]
    fn to_json_round_trips(rule in arb_rule(), data in arb_data()) {
        let data = data.to_string();
        for folding in [true, false] {
            let engine = Engine::builder().with_constant_folding(folding).build();
            let Some(compiled) = compile(&engine, &rule) else { continue };
            let back = compiled.to_json();
            prop_assert!(serde_json::from_str::<Value>(&back).is_ok(), "{rule} -> {back}");
            prop_assert_eq!(
                run(&engine, &back, &data),
                run(&engine, &rule.to_string(), &data),
                "folding {}: {} -> {}", folding, rule, back
            );
        }
    }

    /// The pointer `resolve_node_ids` rebuilds for each node of a rule
    /// compiled without folding is the one a traced compile records. Both
    /// compiles hand out ids in the same order, so ids pair up.
    ///
    /// Arguments are always listed: the compiled tree does not keep
    /// whether a lone argument was written without its array, so a
    /// rebuilt pointer names it as item 0.
    #[test]
    fn resolve_path_matches_traced_pointers(rule in arb_rule().prop_map(listed)) {
        let engine = Engine::builder().with_constant_folding(false).build();
        let Some(plain) = compile(&engine, &rule) else { return Ok(()) };
        let traced = engine.trace().compile(&rule).unwrap();
        prop_assert_eq!(plain.to_json(), traced.to_json());
        let mut checked = 0;
        for id in 1..=4096u32 {
            let Some(want) = traced.pointer(id) else { continue };
            for step in plain.resolve_node_ids(&[id]) {
                prop_assert_eq!(&step.json_pointer, want, "{} id {}", rule, id);
                checked += 1;
            }
        }
        prop_assert!(checked > 0, "{}", rule);
    }
}

/// The cases the pointer property turned up, pinned.
#[test]
fn resolve_path_matches_traced_pointers_pinned() {
    let engine = Engine::builder()
        .with_constant_folding(false)
        .with_templating(true)
        .build();
    for rule in [
        json!({"var": ["x", {"+": [1, {"var": "y"}]}]}),
        json!({"a": {"var": "x"}, "b/c": [{"+": [1, 2]}]}),
        json!({"if": [{"var": "x"}, {"cat": ["a", {"var": "y"}]}, null]}),
        json!({"missing_some": [1, ["a", {"var": "k"}]]}),
    ] {
        let plain = engine.compile(&rule).unwrap();
        let traced = engine.trace().compile(&rule).unwrap();
        for id in 1..=64u32 {
            let Some(want) = traced.pointer(id) else {
                continue;
            };
            for step in plain.resolve_node_ids(&[id]) {
                assert_eq!(step.json_pointer, want, "{rule} id {id}");
            }
        }
    }
}
