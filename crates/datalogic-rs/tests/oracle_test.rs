//! The engine against the reference oracle (`tests/oracle/mod.rs`).
//!
//! Every suite case runs through the oracle and through two engines, the
//! default (constant folding, CSE, fast paths) and one built without
//! constant folding, and all three must agree. The suites' own `result` /
//! `error` expectations are checked by `test_jsonlogic.rs`; this test checks
//! that the optimized engine computes what an unoptimised reading of the
//! rule computes, which is the property every optimizer pass and fast path
//! has to keep.
#![cfg(all(
    feature = "all-operators",
    feature = "serde_json",
    feature = "templating"
))]

mod oracle;

use std::collections::HashMap;
use std::fs;

use datalogic_rs::{Engine, Error, ErrorKind, EvaluationConfig, MissingVar, ScopedArg};
use datavalue::OwnedDataValue as V;
use oracle::Oracle;
use serde_json::Value;

/// The comparable form of an outcome: a value as its JSON text; an error as
/// its kind, plus the payload the suites' `error` expectations read.
fn key(outcome: &Result<V, Error>) -> String {
    match outcome {
        Ok(v) => format!("ok {}", v.to_json_string()),
        Err(e) => {
            let kind = format!("{:?}", e.kind);
            let name = kind
                .split(['(', ' ', '{'])
                .next()
                .unwrap_or_default()
                .to_string();
            match &e.kind {
                ErrorKind::Thrown(v) => format!("err Thrown {}", v.to_json_string()),
                ErrorKind::InvalidArguments(m) => format!("err InvalidArguments {m}"),
                _ => format!("err {name}"),
            }
        }
    }
}

fn run_engine(engine: &Engine, rule: &V, data: &V) -> Result<V, Error> {
    let logic = engine.compile(rule)?;
    engine.session().eval(&logic, data)
}

#[derive(Default)]
struct Flavours {
    /// `MissingVar::Error` for every engine and oracle.
    missing_var_error: bool,
    engines: HashMap<(bool, Option<char>, bool), Engine>,
    oracles: HashMap<(bool, Option<char>), Oracle>,
}

impl Flavours {
    fn engine(&mut self, templating: bool, escape: Option<char>, folding: bool) -> &Engine {
        let missing_var = if self.missing_var_error {
            MissingVar::Error
        } else {
            MissingVar::Null
        };
        self.engines
            .entry((templating, escape, folding))
            .or_insert_with(|| {
                let mut b = Engine::builder()
                    .with_templating(templating)
                    .with_constant_folding(folding)
                    .with_config(EvaluationConfig::default().with_missing_var(missing_var));
                if let Some(c) = escape {
                    b = b.with_template_key_escape(c);
                }
                b.build()
            })
    }

    fn oracle(&mut self, templating: bool, escape: Option<char>) -> &Oracle {
        let missing_var_error = self.missing_var_error;
        self.oracles.entry((templating, escape)).or_insert_with(|| {
            Oracle::with_missing_var_error(templating, escape, missing_var_error)
        })
    }
}

/// Whether the rule calls `now`, whose value changes between evaluations.
fn reads_clock(rule: &Value) -> bool {
    match rule {
        Value::Object(map) => {
            (map.len() == 1 && map.contains_key("now")) || map.values().any(reads_clock)
        }
        Value::Array(items) => items.iter().any(reads_clock),
        _ => false,
    }
}

/// One oracle disagreement.
struct Split {
    case: String,
    oracle: String,
    folded: String,
    unfolded: String,
}

fn suite_cases() -> Vec<(String, usize, Value)> {
    let index = fs::read_to_string("tests/suites/index.json").expect("read index.json");
    let files: Vec<String> = serde_json::from_str(&index).expect("parse index.json");
    let mut out = Vec::new();
    for file in files {
        let text = fs::read_to_string(format!("tests/suites/{file}")).expect("read suite");
        let suite: Value = serde_json::from_str(&text).expect("parse suite");
        for (i, case) in suite.as_array().into_iter().flatten().enumerate() {
            if case.is_object() {
                out.push((file.clone(), i, case.clone()));
            }
        }
    }
    out
}

#[test]
fn every_suite_case_agrees_with_the_oracle() {
    suite_cases_agree(Flavours::default());
}

/// The same with `MissingVar::Error`, where every read that finds nothing
/// raises: the suites' expectations do not apply, but the engine and the
/// oracle must still agree.
#[test]
fn every_suite_case_agrees_with_the_oracle_when_a_miss_is_an_error() {
    suite_cases_agree(Flavours {
        missing_var_error: true,
        ..Flavours::default()
    });
}

fn suite_cases_agree(mut flavours: Flavours) {
    let mut splits = Vec::new();
    let mut checked = 0usize;
    for (file, index, case) in suite_cases() {
        let rule_json = &case["rule"];
        if reads_clock(rule_json) {
            continue;
        }
        let templating = case
            .get("templating")
            .and_then(Value::as_bool)
            .unwrap_or(false);
        let escape = case
            .get("template_key_escape")
            .and_then(Value::as_str)
            .and_then(|s| s.chars().next());
        let rule = V::from_json(&rule_json.to_string()).expect("rule");
        let data = V::from_json(&case["data"].to_string()).expect("data");

        let expected = key(&flavours.oracle(templating, escape).evaluate(&rule, &data));
        let folded = key(&run_engine(
            flavours.engine(templating, escape, true),
            &rule,
            &data,
        ));
        let unfolded = key(&run_engine(
            flavours.engine(templating, escape, false),
            &rule,
            &data,
        ));
        checked += 1;
        if folded != expected || unfolded != expected {
            let description = case["description"].as_str().unwrap_or_default();
            splits.push(Split {
                case: format!(
                    "{file} #{index} {description}: {rule_json} on {}",
                    case["data"]
                ),
                oracle: expected,
                folded,
                unfolded,
            });
        }
    }
    for s in &splits {
        println!(
            "{}\n  oracle:   {}\n  folded:   {}\n  unfolded: {}",
            s.case, s.oracle, s.folded, s.unfolded
        );
    }
    assert!(checked > 1500, "only {checked} cases checked");
    assert!(
        splits.is_empty(),
        "{} of {checked} suite cases disagree with the oracle",
        splits.len()
    );
}

/// The oracle hands every operator it does not implement to the engine,
/// which is only sound for an operator that computes a value from its
/// arguments: one that pushes a frame, reads the current data or catches
/// errors would see the engine's scope instead of the oracle's. A new
/// operator of that kind has to be implemented in the oracle first.
#[test]
fn delegation_is_sound() {
    for op in Engine::new().operators() {
        let implemented = std::iter::once(op.name)
            .chain(op.aliases.iter().copied())
            .any(|n| oracle::IMPLEMENTED.contains(&n));
        if implemented {
            continue;
        }
        assert_eq!(
            op.scoped_arg, None::<ScopedArg>,
            "`{}` runs an argument under a frame; implement it in the oracle",
            op.name
        );
        assert!(
            !matches!(op.effect, "catches" | "throws"),
            "`{}` has effect `{}`; implement it in the oracle",
            op.name,
            op.effect
        );
        assert!(
            !op.reads_context || oracle::ROOT_READERS.contains(&op.name),
            "`{}` reads the data context; implement it in the oracle",
            op.name
        );
    }
}

// ---------------------------------------------------------------------------
// Generated rules
// ---------------------------------------------------------------------------
//
// Rules built from the operators the optimizer rewrites, in the shapes its
// fast paths recognise (`filter` on a field compared with a literal, `map`
// and `reduce` arithmetic over fields, `reduce` over a `map`, `sort` on a
// field, repeated subexpressions for CSE, literal subtrees for folding),
// over data whose fields are sometimes missing, null, mistyped or near the
// edge of `i64`. Each one must evaluate the same in the oracle, the default
// engine and an engine without constant folding.
//
// 512 cases per run by default; override with `PROPTEST_CASES`.

use proptest::prelude::*;
use proptest::sample::select;
use serde_json::json;

/// Where an expression is evaluated: the scope its paths read.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Ctx {
    /// The root data object.
    Root,
    /// An iterator body over `objs` / `xs` / `o`: the element.
    Item,
    /// A `reduce` body.
    Reduce,
}

fn arb_lit() -> BoxedStrategy<Value> {
    prop_oneof![
        Just(json!(null)),
        any::<bool>().prop_map(Value::from),
        (-3i64..4).prop_map(Value::from),
        select(vec![
            json!(1.5),
            json!(-0.5),
            json!(2.0),
            json!(i64::MAX),
            json!(i64::MIN),
            json!(9007199254740993i64),
        ]),
        select(vec!["a", "b", "", "1", "a.b", "k", "x"]).prop_map(Value::from),
    ]
    .boxed()
}

fn arb_leaf(ctx: Ctx) -> BoxedStrategy<Value> {
    let root_path = select(vec![
        "n", "f", "s", "b", "z", "xs", "objs", "o", "o.a", "o.a.b", "xs.0", "objs.1.k", "nope", "",
        "a.b",
    ]);
    let root = prop_oneof![
        root_path.clone().prop_map(|p| json!({"var": p})),
        (root_path.clone(), arb_lit()).prop_map(|(p, d)| json!({"var": [p, d]})),
        root_path.prop_map(|p| json!({"val": p})),
        select(vec![
            json!(["o", "a"]),
            json!(["objs", 0, "k"]),
            json!(["xs", 1])
        ])
        .prop_map(|p| json!({"val": p})),
        select(vec!["n", "o.a", "nope"]).prop_map(|p| json!({"exists": p})),
        select(vec![
            json!(["n", "nope"]),
            json!("o.a.b"),
            json!(["s", "z"])
        ])
        .prop_map(|p| json!({"missing": p})),
        (0i64..3).prop_map(|m| json!({"missing_some": [m, ["n", "nope", "s"]]})),
    ];
    let mut options = vec![(3, arb_lit()), (3, root.boxed())];
    match ctx {
        Ctx::Root => {}
        Ctx::Item => {
            let item = prop_oneof![
                select(vec!["", "k", "name", "nope", "k.x", "0"]).prop_map(|p| json!({"var": p})),
                select(vec![
                    json!({"val": [[1], "index"]}),
                    json!({"val": [[1], "key"]}),
                    json!({"val": [[0], "index"]}),
                    json!({"val": [[2], "n"]}),
                    json!({"val": [[3], "index"]}),
                    json!({"val": [[3], "key"]}),
                    json!({"val": [[4], "n"]}),
                    json!({"val": [[2], "name"]}),
                    json!({"var": "type"}),
                    json!({"val": "k"}),
                    json!({"exists": "k"}),
                    json!({"missing": ["k", "name"]}),
                ]),
            ];
            options.push((6, item.boxed()));
        }
        Ctx::Reduce => {
            let reduce = select(vec![
                json!({"var": "current"}),
                json!({"var": "accumulator"}),
                json!({"var": "current.k"}),
                json!({"val": ["current", "k"]}),
                json!({"val": "current"}),
            ]);
            options.push((6, reduce.boxed()));
        }
    }
    proptest::strategy::Union::new_weighted(options).boxed()
}

/// A subtree with no data reads, which the default engine folds into a
/// literal before its parent sees it.
fn arb_static() -> BoxedStrategy<Value> {
    let lit = arb_lit();
    prop_oneof![
        (lit.clone(), lit.clone()).prop_map(|(a, b)| json!({"merge": [[a], [b]]})),
        (lit.clone(), lit.clone()).prop_map(|(a, b)| json!({"cat": [a, b]})),
        (lit.clone(), lit.clone()).prop_map(|(a, b)| json!({"+": [a, b]})),
        (lit.clone(), lit.clone(), lit.clone()).prop_map(|(c, a, b)| json!({"if": [c, [a, b], b]})),
        Just(json!({"distinct": [null]})),
        Just(json!({"if": [true, null, 1]})),
        Just(json!({"cat": ["o", ".a"]})),
        Just(json!({"cat": ["a", ".b"]})),
        Just(json!({"merge": [[[1], "x"], [[2], "y"]]})),
        Just(json!({"/": [3, 2]})),
        Just(json!({"merge": [[1]]})),
        lit,
    ]
    .boxed()
}

/// An iterator source.
fn arb_source(sub: BoxedStrategy<Value>) -> BoxedStrategy<Value> {
    prop_oneof![
        6 => select(vec!["objs", "xs", "o", "z", "n", "nope", "s"]).prop_map(|p| json!({"var": p})),
        // inside an iterator: the element's own array, or the root's, two
        // levels up
        2 => select(vec![
            json!({"var": "sub"}),
            json!({"var": ""}),
            json!({"val": [[2], "objs"]}),
            json!({"val": [[2], "xs"]}),
        ]),
        1 => sub,
    ]
    .boxed()
}

fn arb_expr(ctx: Ctx, depth: u32) -> BoxedStrategy<Value> {
    let leaf = arb_leaf(ctx);
    if depth == 0 {
        return leaf;
    }
    let sub = arb_expr(ctx, depth - 1);
    let item = arb_expr(Ctx::Item, depth - 1);
    let reduce = arb_expr(Ctx::Reduce, depth - 1);
    let lit = arb_lit();
    let source = arb_source(sub.clone());
    let binary = select(vec![
        "+", "-", "*", "/", "%", "max", "min", "==", "===", "!=", "!==", "<", "<=", ">", ">=",
        "and", "or", "??", "cat", "in", "merge",
    ]);
    let compare = select(vec!["==", "===", "!=", "!==", "<", "<=", ">", ">="]);
    let arith = select(vec!["+", "-", "*"]);
    let quantifier = select(vec!["filter", "all", "some", "none"]);

    let ops = prop_oneof![
        // plain operator calls
        4 => (binary, sub.clone(), sub.clone()).prop_map(|(op, a, b)| json!({op: [a, b]})),
        1 => (select(vec!["<", "<="]), sub.clone(), sub.clone(), sub.clone())
            .prop_map(|(op, a, b, c)| json!({op: [a, b, c]})),
        1 => (select(vec!["!", "!!", "length", "+", "-"]), sub.clone()).prop_map(|(op, a)| json!({op: [a]})),
        2 => (sub.clone(), sub.clone(), sub.clone()).prop_map(|(a, b, c)| json!({"if": [a, b, c]})),
        1 => (sub.clone(), lit.clone(), sub.clone(), lit.clone(), sub.clone(), sub.clone())
            .prop_map(|(d, c1, r1, c2, r2, def)| json!({"switch": [d, [[c1, r1], [c2, r2]], def]})),
        1 => (sub.clone(), sub.clone()).prop_map(|(a, b)| json!({"try": [a, b]})),
        1 => (sub.clone(), item.clone()).prop_map(|(a, b)| json!({"try": [a, {"throw": "x"}, b]})),
        1 => sub.clone().prop_map(|a| json!({"try": [a, {"var": "type"}]})),
        1 => (sub.clone(), sub.clone()).prop_map(|(a, b)| json!({"try": [{"/": [a, b]}, {"cat": ["caught ", {"var": "type"}]}]})),
        // a repeated subexpression (CSE) and a literal one (folding)
        1 => sub.clone().prop_map(|e| json!({"+": [e.clone(), e]})),
        1 => sub.clone().prop_map(|e| json!({"if": [e.clone(), e, null]})),
        1 => (lit.clone(), lit.clone()).prop_map(|(a, b)| json!({"cat": [a, {"+": [b, 1]}]})),
        // iterators with general bodies
        3 => (source.clone(), item.clone()).prop_map(|(s, b)| json!({"map": [s, b]})),
        3 => (quantifier.clone(), source.clone(), item.clone()).prop_map(|(q, s, b)| json!({q: [s, b]})),
        2 => (source.clone(), reduce.clone(), sub.clone())
            .prop_map(|(s, b, i)| json!({"reduce": [s, b, i]})),
        1 => (source.clone(), any::<bool>(), item.clone())
            .prop_map(|(s, d, k)| json!({"sort": [s, d, k]})),
        1 => source.clone().prop_map(|s| json!({"sort": [s]})),
        1 => (source.clone(), item.clone()).prop_map(|(s, k)| json!({"group_by": [s, k]})),
        1 => source.clone().prop_map(|s| json!({"distinct": [s]})),
        // the fast paths' shapes
        3 => (quantifier, source.clone(), compare, select(vec!["", "k", "nope"]), lit.clone(), any::<bool>())
            .prop_map(|(q, s, c, p, l, flip)| {
                let field = json!({"var": p});
                let args = if flip { json!([l, field]) } else { json!([field, l]) };
                json!({q: [s, {c: args}]})
            }),
        3 => (source.clone(), arith.clone(), select(vec!["", "k"]), lit.clone(), any::<bool>())
            .prop_map(|(s, op, p, l, flip)| {
                let field = json!({"var": p});
                let args = if flip { json!([l, field]) } else { json!([field, l]) };
                json!({"map": [s, {op: args}]})
            }),
        1 => (source.clone(), arith.clone()).prop_map(|(s, op)| json!({"map": [s, {op: [{"var": "k"}, {"var": "k"}]}]})),
        2 => (source.clone(), arith.clone(), lit.clone()).prop_map(|(s, op, init)| {
            json!({"reduce": [s, {op: [{"var": "current"}, {"var": "accumulator"}]}, init]})
        }),
        2 => (source.clone(), arith.clone(), lit.clone()).prop_map(|(s, op, init)| {
            json!({"reduce": [{"map": [s, {"var": "k"}]}, {op: [{"var": "accumulator"}, {"var": "current"}]}, init]})
        }),
        1 => (source, any::<bool>(), select(vec!["k", "name", "nope"]))
            .prop_map(|(s, d, p)| json!({"sort": [s, d, {"var": p}]})),
    ];
    let fixed = arb_static();
    let shaped = prop_oneof![
        // arguments whose literal shape an operator or compile hook reads
        (select(vec!["+", "*", "max", "min"]), fixed.clone()).prop_map(|(op, a)| json!({op: [a]})),
        (fixed.clone(), lit.clone()).prop_map(|(p, d)| json!({"var": [p, d]})),
        fixed.clone().prop_map(|p| json!({"var": p})),
        fixed.clone().prop_map(|p| json!({"val": [p]})),
        (fixed.clone(), fixed.clone()).prop_map(|(a, b)| json!({"val": [a, b]})),
        fixed.clone().prop_map(|s| json!({"sort": [s]})),
        (fixed.clone(), fixed.clone()).prop_map(|(a, b)| json!({"slice": [{"var": "xs"}, a, b]})),
        (fixed.clone(), sub.clone()).prop_map(|(c, d)| json!({"switch": [1, c, d]})),
        (fixed.clone(), sub.clone())
            .prop_map(|(c, d)| json!({"switch": ["x", [c, [2, "two"]], d]})),
        (
            fixed.clone(),
            select(vec![json!(["n", "nope", "s"]), json!(["nope", "z", "q"])])
        )
            .prop_map(|(m, p)| json!({"missing_some": [m, p]})),
        select(vec![json!(1.5), json!(2.5), json!(-1.0), json!(2.0)])
            .prop_map(|m| json!({"missing_some": [m, ["n", "nope", "q", "s"]]})),
        (fixed.clone(), fixed.clone()).prop_map(|(a, b)| json!({"substr": ["abcdef", a, b]})),
        (fixed.clone(), sub.clone()).prop_map(|(a, b)| json!({"in": [a, b]})),
        fixed.clone().prop_map(|a| json!({"missing": [a]})),
        fixed.prop_map(|a| json!({"exists": a})),
    ];
    prop_oneof![2 => leaf, 3 => ops, 1 => shaped].boxed()
}

fn arb_scalar() -> BoxedStrategy<Value> {
    prop_oneof![
        3 => (-3i64..4).prop_map(Value::from),
        1 => select(vec![json!(i64::MAX), json!(i64::MIN), json!(1.5), json!(-0.0), json!(1e300)]),
        1 => select(vec!["a", "b", "", "1"]).prop_map(Value::from),
        1 => Just(json!(null)),
        1 => any::<bool>().prop_map(Value::from),
    ]
    .boxed()
}

fn arb_data() -> BoxedStrategy<Value> {
    let obj = (prop::option::of(arb_scalar()), select(vec!["a", "b", "c"])).prop_map(
        |(k, name)| match k {
            Some(k) => json!({"k": k, "name": name, "sub": [k, 1]}),
            None => json!({"name": name}),
        },
    );
    (
        arb_scalar(),
        arb_scalar(),
        prop::collection::vec(arb_scalar(), 0..5),
        prop::collection::vec(obj, 0..5),
        arb_scalar(),
        arb_scalar(),
    )
        .prop_map(|(n, f, xs, objs, ab, k)| {
            json!({
                "n": n, "f": f, "s": "ab", "b": true, "z": null,
                "xs": xs, "objs": objs,
                "o": {"a": {"b": ab}, "k": k},
                "a.b": 7,
            })
        })
        .boxed()
}

fn check_agrees(oracle: &Oracle, folded: &Engine, unfolded: &Engine, rule: &Value, data: &Value) {
    let rule = V::from_json(&rule.to_string()).expect("rule");
    let data = V::from_json(&data.to_string()).expect("data");
    let expected = key(&oracle.evaluate(&rule, &data));
    let got_folded = key(&run_engine(folded, &rule, &data));
    let got_unfolded = key(&run_engine(unfolded, &rule, &data));
    assert_eq!(
        (&got_folded, &got_unfolded),
        (&expected, &expected),
        "rule {} on {}",
        rule.to_json_string(),
        data.to_json_string()
    );
}

thread_local! {
    static ENGINES: (Oracle, Engine, Engine) = (
        Oracle::new(false, None),
        Engine::new(),
        Engine::builder().with_constant_folding(false).build(),
    );
    static MISSING_VAR_ERROR: (Oracle, Engine, Engine) = {
        let config = || EvaluationConfig::default().with_missing_var(MissingVar::Error);
        (
            Oracle::with_missing_var_error(false, None, true),
            Engine::builder().with_config(config()).build(),
            Engine::builder()
                .with_config(config())
                .with_constant_folding(false)
                .build(),
        )
    };
}

/// A rule in templating mode: an output object over generated
/// expressions, with an escaped key, an unknown single key and an operator
/// key the escape turns into a field.
fn arb_template() -> BoxedStrategy<Value> {
    let e = arb_expr(Ctx::Root, 2);
    (e.clone(), e.clone(), e.clone(), e)
        .prop_map(|(a, b, c, d)| {
            json!({
                "out": a,
                "$type": b,
                "list": [c, {"unknown_key": d}],
                "nested": {"$$x": 1, "y": {"var": "n"}},
            })
        })
        .boxed()
}

thread_local! {
    static TEMPLATING: (Oracle, Engine, Engine) = (
        Oracle::new(true, Some('$')),
        Engine::builder().with_templating(true).with_template_key_escape('$').build(),
        Engine::builder()
            .with_templating(true)
            .with_template_key_escape('$')
            .with_constant_folding(false)
            .build(),
    );
}

proptest! {
    #![proptest_config(ProptestConfig {
        cases: std::env::var("PROPTEST_CASES").ok().and_then(|v| v.parse().ok()).unwrap_or(512),
        ..ProptestConfig::default()
    })]

    #[test]
    fn generated_rules_agree_with_the_oracle(rule in arb_expr(Ctx::Root, 3), data in arb_data()) {
        ENGINES.with(|(oracle, folded, unfolded)| check_agrees(oracle, folded, unfolded, &rule, &data));
    }

    #[test]
    fn generated_rules_agree_with_the_oracle_when_a_miss_is_an_error(
        rule in arb_expr(Ctx::Root, 3),
        data in arb_data(),
    ) {
        MISSING_VAR_ERROR.with(|(oracle, folded, unfolded)| check_agrees(oracle, folded, unfolded, &rule, &data));
    }

    #[test]
    fn generated_templates_agree_with_the_oracle(rule in arb_template(), data in arb_data()) {
        TEMPLATING.with(|(oracle, folded, unfolded)| check_agrees(oracle, folded, unfolded, &rule, &data));
    }
}

/// Exploration aid, not a check: draw `ORACLE_EXPLORE` generated cases (or
/// 20,000) and print the shortest disagreements, without stopping at the
/// first one.
///
/// ```text
/// cargo test -p datalogic-rs --all-features --test oracle_test explore -- --ignored --nocapture
/// ```
#[test]
#[ignore]
fn explore_disagreements() {
    use proptest::strategy::ValueTree;
    use proptest::test_runner::TestRunner;
    let n: usize = std::env::var("ORACLE_EXPLORE")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(20_000);
    let mut runner = TestRunner::deterministic();
    let strategy = (arb_expr(Ctx::Root, 3), arb_data());
    let mut found: Vec<(String, String)> = Vec::new();
    ENGINES.with(|(oracle, folded, unfolded)| {
        for _ in 0..n {
            let (rule, data) = strategy.new_tree(&mut runner).unwrap().current();
            if std::env::var_os("ORACLE_SHOW").is_some() {
                println!("{rule}");
            }
            let r = V::from_json(&rule.to_string()).unwrap();
            let d = V::from_json(&data.to_string()).unwrap();
            let expected = key(&oracle.evaluate(&r, &d));
            let f = key(&run_engine(folded, &r, &d));
            let u = key(&run_engine(unfolded, &r, &d));
            if f != expected || u != expected {
                found.push((
                    rule.to_string(),
                    format!("data {data}\n  oracle {expected}\n  folded {f}\n  unfolded {u}"),
                ));
            }
        }
    });
    found.sort_by_key(|(r, _)| r.len());
    found.dedup_by(|a, b| a.0 == b.0);
    println!("{} disagreements", found.len());
    for (rule, detail) in found.iter().take(400) {
        println!("{rule}\n  {detail}");
    }
}
