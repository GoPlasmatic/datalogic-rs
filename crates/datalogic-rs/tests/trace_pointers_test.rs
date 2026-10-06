//! Trace node pointers: a rule compiled for tracing records, for every node
//! id, the RFC 6901 JSON Pointer of the source value the node was compiled
//! from. A debugger maps trace steps to the rule it shows through them,
//! instead of re-deriving the compiler's canonical forms.

#![cfg(all(feature = "trace", feature = "serde_json"))]

use std::collections::BTreeSet;

use datalogic_rs::{Engine, ExpressionNode, Logic};
use serde_json::{Value, json};

fn traced(engine: &Engine, rule: &Value) -> Logic {
    engine.trace().compile(rule).unwrap()
}

fn tree(engine: &Engine, logic: &Logic) -> ExpressionNode {
    engine.trace().eval(logic, "null").expression_tree
}

/// Every pointer in the tree, by node id order of a pre-order walk.
fn pointers(logic: &Logic, node: &ExpressionNode, out: &mut Vec<(u32, String)>) {
    let p = logic
        .pointer(node.id)
        .unwrap_or_else(|| panic!("node {} has no pointer", node.id));
    out.push((node.id, p.to_string()));
    for child in &node.children {
        pointers(logic, child, out);
    }
}

fn pointer_of(engine: &Engine, rule: Value, at: &str) -> String {
    // The pointer of the tree node whose expression parses to `at`.
    let logic = traced(engine, &rule);
    let root = tree(engine, &logic);
    let mut all = Vec::new();
    pointers(&logic, &root, &mut all);
    let want: Value = serde_json::from_str(at).unwrap();
    let mut found = None;
    walk(&root, &mut |n| {
        if serde_json::from_str::<Value>(&n.expression).ok().as_ref() == Some(&want) {
            found = Some(logic.pointer(n.id).unwrap().to_string());
        }
    });
    found.unwrap_or_else(|| panic!("no node {at} in {rule}"))
}

fn walk(node: &ExpressionNode, f: &mut impl FnMut(&ExpressionNode)) {
    f(node);
    for c in &node.children {
        walk(c, f);
    }
}

/// Each node's pointer resolves in the rule, sits strictly below its
/// parent's, and no two children of one node share one.
fn assert_well_placed(rule: &Value, logic: &Logic, node: &ExpressionNode, what: &str) {
    let here = logic.pointer(node.id).unwrap();
    assert!(
        rule.pointer(here).is_some(),
        "{what}: pointer {here:?} does not resolve in {rule}"
    );
    let mut seen = BTreeSet::new();
    for child in &node.children {
        let p = logic.pointer(child.id).unwrap();
        assert!(
            p.len() > here.len() && p.starts_with(here) && p.as_bytes()[here.len()] == b'/',
            "{what}: child pointer {p:?} is not below {here:?} in {rule}"
        );
        assert!(seen.insert(p), "{what}: two children at {p:?} in {rule}");
        assert_well_placed(rule, logic, child, what);
    }
}

#[test]
fn the_root_is_the_whole_rule() {
    let engine = Engine::new();
    let rule = json!({"+": [1, {"var": "x"}]});
    let logic = traced(&engine, &rule);
    let root = tree(&engine, &logic);
    assert_eq!(logic.pointer(root.id), Some(""));
}

#[test]
fn arguments_are_indexed_under_their_operator() {
    let engine = Engine::new();
    let rule =
        json!({"if": [{"==": [{"var": "a"}, 1]}, {"var": "b"}, {"cat": ["x", {"var": "c"}]}]});
    assert_eq!(
        pointer_of(&engine, rule.clone(), r#"{"==":[{"var":"a"},1]}"#),
        "/if/0"
    );
    assert_eq!(
        pointer_of(&engine, rule.clone(), r#"{"var":"a"}"#),
        "/if/0/==/0"
    );
    assert_eq!(pointer_of(&engine, rule.clone(), r#"{"var":"b"}"#), "/if/1");
    assert_eq!(pointer_of(&engine, rule, r#"{"var":"c"}"#), "/if/2/cat/1");
}

#[test]
fn a_lone_argument_is_the_operator_value() {
    let engine = Engine::new();
    let rule = json!({"!": {"var": "x"}});
    assert_eq!(pointer_of(&engine, rule, r#"{"var":"x"}"#), "/!");
}

#[test]
fn an_alias_keeps_the_key_as_written() {
    let engine = Engine::new();
    let rule = json!({"?:": [true, {"var": "x"}, 0]});
    assert_eq!(pointer_of(&engine, rule, r#"{"var":"x"}"#), "/?:/1");
}

#[test]
fn a_var_default_is_its_second_argument() {
    let engine = Engine::new();
    let rule = json!({"var": ["x", {"+": [1, 2]}]});
    assert_eq!(pointer_of(&engine, rule, r#"{"+":[1,2]}"#), "/var/1");
}

#[test]
fn a_computed_path_is_the_var_argument() {
    let engine = Engine::new();
    let rule = json!({"var": {"cat": ["a", ".b"]}});
    assert_eq!(pointer_of(&engine, rule, r#"{"cat":["a",".b"]}"#), "/var");
}

#[test]
fn an_array_argument_indexes_its_items() {
    let engine = Engine::new();
    let rule = json!({"in": [{"var": "x"}, [1, {"var": "y"}]]});
    assert_eq!(pointer_of(&engine, rule, r#"{"var":"y"}"#), "/in/1/1");
}

#[test]
fn iterator_bodies_are_their_argument() {
    let engine = Engine::new();
    let rule = json!({"map": [{"var": "xs"}, {"*": [{"var": ""}, 2]}]});
    assert_eq!(
        pointer_of(&engine, rule.clone(), r#"{"*":[{"var":""},2]}"#),
        "/map/1"
    );
    assert_eq!(pointer_of(&engine, rule, r#"{"var":""}"#), "/map/1/*/0");
}

#[test]
fn a_custom_operator_indexes_its_arguments() {
    use datalogic_rs::operator::EvalContext;
    use datalogic_rs::{CustomOperator, DataValue, Result};
    struct First;
    impl CustomOperator for First {
        fn evaluate<'a>(
            &self,
            args: &[&'a DataValue<'a>],
            _ctx: &mut EvalContext<'_, 'a>,
            _arena: &'a datalogic_rs::bumpalo::Bump,
        ) -> Result<&'a DataValue<'a>> {
            Ok(args[0])
        }
    }
    let engine = Engine::builder().add_operator("first", First).build();
    let rule = json!({"first": [{"var": "a"}, {"var": "b"}]});
    assert_eq!(pointer_of(&engine, rule, r#"{"var":"b"}"#), "/first/1");
}

#[cfg(feature = "templating")]
#[test]
fn template_fields_are_their_keys() {
    let engine = Engine::builder()
        .with_templating(true)
        .with_template_key_escape('$')
        .build();
    let rule = json!({"a/b": {"var": "x"}, "c~d": [{"var": "y"}], "$if": {"var": "z"}});
    assert_eq!(pointer_of(&engine, rule.clone(), r#"{"var":"x"}"#), "/a~1b");
    assert_eq!(
        pointer_of(&engine, rule.clone(), r#"{"var":"y"}"#),
        "/c~0d/0"
    );
    assert_eq!(pointer_of(&engine, rule, r#"{"var":"z"}"#), "/$if");
}

#[test]
fn every_step_has_a_pointer() {
    let engine = Engine::new();
    let rule = json!({"reduce": [{"filter": [{"var": "xs"}, {">": [{"var": ""}, 1]}]}, {"+": [{"var": "current"}, {"var": "accumulator"}]}, 0]});
    let logic = traced(&engine, &rule);
    let run = engine.trace().eval(&logic, r#"{"xs": [1, 2, 3]}"#);
    assert_eq!(run.result.unwrap().to_json_string(), "5");
    assert!(!run.steps.is_empty());
    for step in &run.steps {
        let p = logic
            .pointer(step.node_id)
            .unwrap_or_else(|| panic!("step {step:?}"));
        assert!(rule.pointer(p).is_some(), "{p}");
    }
}

#[test]
fn a_traced_compile_runs_like_the_one_shot_trace() {
    let engine = Engine::new();
    let rule = json!({"if": [{"var": "a"}, {"+": [1, 2]}, "no"]});
    let logic = traced(&engine, &rule);
    let compiled = engine.trace().eval(&logic, r#"{"a": true}"#);
    let one_shot = engine.trace().eval_str(&rule, r#"{"a": true}"#);
    assert_eq!(compiled.result.unwrap().to_json_string(), "3");
    assert_eq!(
        serde_json::to_value(&compiled.expression_tree).unwrap(),
        serde_json::to_value(&one_shot.expression_tree).unwrap()
    );
    assert_eq!(compiled.steps.len(), one_shot.steps.len());
}

#[test]
fn an_ordinary_compile_records_none() {
    let engine = Engine::new();
    let logic = engine.compile(r#"{"var": "x"}"#).unwrap();
    assert_eq!(logic.pointer(1), None);
    assert_eq!(logic.pointers().count(), 0);
}

#[test]
fn every_recorded_pointer_is_listed_in_id_order() {
    let engine = Engine::new();
    let logic = traced(&engine, &json!({"+": [1, {"var": "x"}]}));
    let all: Vec<(u32, &str)> = logic.pointers().collect();
    // The literal `1`, the read's path literal (compiled before the read
    // takes it over), the read and the call, each at its place.
    assert_eq!(all, [(1, "/+/0"), (2, "/+/1/var"), (3, "/+/1"), (4, "")]);
    for (id, p) in &all {
        assert_eq!(logic.pointer(*id), Some(*p));
    }
}

#[test]
fn an_unknown_id_has_none() {
    let engine = Engine::new();
    let logic = traced(&engine, &json!({"var": "x"}));
    assert_eq!(logic.pointer(0), None);
    assert_eq!(logic.pointer(10_000), None);
}

#[test]
fn a_compile_error_is_returned() {
    let engine = Engine::new();
    assert!(engine.trace().compile(r#"{"a": 1, "b": 2}"#).is_err());
}

#[test]
fn every_suite_rule_places_its_nodes() {
    let index = std::fs::read_to_string("tests/suites/index.json").unwrap();
    let files: Vec<String> = serde_json::from_str(&index).unwrap();
    let strict = Engine::new();
    #[cfg(feature = "templating")]
    let templating = Engine::builder().with_templating(true).build();
    let mut placed = 0;
    for file in files {
        let text = std::fs::read_to_string(format!("tests/suites/{file}")).unwrap();
        let suite: Value = serde_json::from_str(&text).unwrap();
        for case in suite.as_array().unwrap().iter().filter(|c| c.is_object()) {
            if case.get("template_key_escape").is_some() {
                continue;
            }
            let templated = case.get("templating").and_then(Value::as_bool) == Some(true);
            #[cfg(not(feature = "templating"))]
            if templated {
                continue;
            }
            #[cfg(feature = "templating")]
            let engine = if templated { &templating } else { &strict };
            #[cfg(not(feature = "templating"))]
            let engine = &strict;
            let rule = &case["rule"];
            let Ok(logic) = engine.trace().compile(rule) else {
                continue;
            };
            let root = tree(engine, &logic);
            let what = format!("{file}: {}", case["description"]);
            assert_eq!(logic.pointer(root.id), Some(""), "{what}");
            assert_well_placed(rule, &logic, &root, &what);
            placed += 1;
        }
    }
    assert!(placed > 1500, "{placed}");
}
