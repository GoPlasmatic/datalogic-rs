//! What the `datalogic-rs` language bindings share: the JSON wire formats
//! they hand to their host languages, and the small pieces of plumbing
//! every binding needs.
//!
//! Each binding (`bindings/wasm`, `node`, `python`, `c`) depends on this
//! crate, so a format is defined once and every host sees the same bytes.
//! The Go, JVM, .NET and PHP wrappers reach the same formats through the C
//! ABI.
//!
//! | Format | Function |
//! |---|---|
//! | Traced run: `{result, expression_tree, steps, error?, structured_error?, pointers?}` | [`traced_json`], [`traced_run_json`] |
//! | Operator catalogue: the schema of `docs/src/operators/operators.json` | [`operators_json`] |
//! | Rule facts: `{reads, computed_reads, reads_complete, reads_data, operators, custom_operators, deterministic}` | [`facts_json`] |
//! | Diagnostics: `[{code, severity, message, pointer, operator}]` | [`diagnostics_json`] |
//! | Custom operator call: arguments as one JSON array, result as JSON | [`args_json`], [`parse_result`] |

use datalogic_rs::bumpalo::Bump;
use std::collections::BTreeMap;

use datalogic_rs::{
    CheckMode, DataValue, Diagnostic, Engine, Error, ExecutionStep, ExpressionNode, Facts, Family,
    IntoLogic, Logic, OperatorInfo, ScopedArg, TracedRun,
};
use serde::Serialize;
use serde_json::{Value, json};

/// The JSON type name of a value, as the typed-result entry points report
/// a mismatch (`"expected a boolean, got string"`).
pub fn type_of(v: &DataValue<'_>) -> &'static str {
    if v.is_null() {
        "null"
    } else if v.is_bool() {
        "boolean"
    } else if v.is_number() {
        "number"
    } else if v.is_string() {
        "string"
    } else if v.is_array() {
        "array"
    } else {
        "object"
    }
}

/// A traced evaluation as the debugger UI reads it:
/// `{result, expression_tree, steps}` plus `error` (the message) and
/// `structured_error` (the serialised [`Error`]) when it failed. `result`
/// is the parsed JSON value, or `null` on failure.
pub fn traced_run_json(run: &TracedRun<String>) -> String {
    traced_wire(run, None)
}

/// Trace `rule` over `data` as the debugger UI reads it: the
/// [`traced_run_json`] envelope plus `pointers`, the JSON Pointer into
/// `rule` of every node id the run can name (`{"3": "/if/1", ...}`), so a
/// host places each step in the rule it shows. A rule that does not compile
/// gives the same failed envelope as [`traced_run_json`], without
/// `pointers`.
pub fn traced_json(engine: &Engine, rule: &str, data: &str) -> String {
    let tracer = engine.trace();
    let logic = match tracer.compile(rule) {
        Ok(logic) => logic,
        // What `eval_str` reports for a rule that does not compile, without
        // compiling it a second time.
        Err(e) => {
            return traced_run_json(&TracedRun {
                result: Err(e),
                steps: Vec::new(),
                expression_tree: ExpressionNode {
                    id: 0,
                    expression: String::new(),
                    children: Vec::new(),
                },
            });
        }
    };
    let run = tracer.eval(&logic, data);
    let run = TracedRun {
        result: run.result.map(|v| v.to_json_string()),
        steps: run.steps,
        expression_tree: run.expression_tree,
    };
    traced_wire(&run, Some(&logic))
}

fn traced_wire(run: &TracedRun<String>, logic: Option<&Logic>) -> String {
    #[derive(Serialize)]
    struct Wire<'a> {
        result: Value,
        expression_tree: &'a ExpressionNode,
        steps: &'a [ExecutionStep],
        #[serde(skip_serializing_if = "Option::is_none")]
        error: Option<String>,
        #[serde(skip_serializing_if = "Option::is_none")]
        structured_error: Option<&'a Error>,
        #[serde(skip_serializing_if = "Option::is_none")]
        pointers: Option<BTreeMap<u32, &'a str>>,
    }

    let (result, error, structured_error) = match &run.result {
        // The string is already JSON; surface the parsed value, or the
        // string itself in the unexpected case that it does not parse.
        Ok(s) => (
            serde_json::from_str::<Value>(s).unwrap_or_else(|_| Value::String(s.clone())),
            None,
            None,
        ),
        Err(e) => (Value::Null, Some(e.to_string()), Some(e)),
    };
    serde_json::to_string(&Wire {
        result,
        expression_tree: &run.expression_tree,
        steps: &run.steps,
        error,
        structured_error,
        pointers: logic.map(|logic| logic.pointers().collect()),
    })
    .unwrap_or_default()
}

/// The largest operation budget a JavaScript number carries exactly.
const MAX_SAFE_BUDGET: f64 = 9_007_199_254_740_991.0;

/// The message for a budget that is not a whole number of operations.
const BUDGET_ERROR: &str = "budget must be a whole number of operations >= 1";

/// Validate a budget a JavaScript host passed as a number: `None` stays
/// `None` (the engine's own budget applies), a whole number from 1 to
/// [`MAX_SAFE_BUDGET`] is taken, anything else is [`BUDGET_ERROR`].
fn budget_from_f64(budget: Option<f64>) -> Result<Option<u64>, &'static str> {
    match budget {
        None => Ok(None),
        Some(n) if n.is_finite() && n >= 1.0 && n.fract() == 0.0 && n <= MAX_SAFE_BUDGET => {
            Ok(Some(n as u64))
        }
        Some(_) => Err(BUDGET_ERROR),
    }
}

/// The operation budget for one call from a JavaScript host: `budget`
/// validated by [`budget_from_f64`], or the engine's own budget when the
/// host passed none ([`Engine::resolve_ops_budget`]).
pub fn resolve_budget(engine: &Engine, budget: Option<f64>) -> Result<u64, &'static str> {
    Ok(engine.resolve_ops_budget(budget_from_f64(budget)?))
}

/// A custom operator's arguments as one JSON array, the form every host
/// callback receives.
pub fn args_json(args: &[&DataValue<'_>]) -> String {
    let mut json = String::from("[");
    for (i, a) in args.iter().enumerate() {
        if i > 0 {
            json.push(',');
        }
        json.push_str(&a.to_json_string());
    }
    json.push(']');
    json
}

/// Parse the JSON a host callback returned for custom operator `name` into
/// the arena.
pub fn parse_result<'a>(
    name: &str,
    json: &str,
    arena: &'a Bump,
) -> datalogic_rs::Result<&'a DataValue<'a>> {
    let text = arena.alloc_str(json);
    let parsed = DataValue::from_str(text, arena).map_err(|e| {
        Error::custom_message(format!(
            "custom operator '{name}' returned invalid JSON: {e}"
        ))
    })?;
    Ok(arena.alloc(parsed))
}

/// One built-in operator in the catalogue schema, keys in catalogue order.
#[derive(Serialize)]
struct OperatorJson<'a> {
    name: &'a str,
    aliases: &'a [&'a str],
    family: &'a str,
    feature: Option<&'a str>,
    min_args: usize,
    max_args: Option<usize>,
    reads_context: bool,
    effect: &'a str,
    cost: &'a str,
    scoped_arg: Value,
}

impl<'a> From<&'a OperatorInfo> for OperatorJson<'a> {
    fn from(op: &'a OperatorInfo) -> Self {
        let scoped_arg = match op.scoped_arg {
            None => Value::Null,
            Some(ScopedArg::Index(i)) => json!(i),
            Some(ScopedArg::LastOfMany) => json!("last"),
            // A later kind of scoping: name it rather than drop it.
            Some(other) => json!(format!("{other:?}")),
        };
        OperatorJson {
            name: op.name,
            aliases: op.aliases,
            family: op.family,
            feature: op.feature,
            min_args: op.min_args,
            max_args: op.max_args,
            reads_context: op.reads_context,
            effect: op.effect,
            cost: op.cost,
            scoped_arg,
        }
    }
}

/// Every built-in operator `engine` evaluates
/// ([`Engine::operators`]), as a JSON array in the catalogue schema.
pub fn operators_json(engine: &Engine) -> String {
    with_operators(engine, |rows| serde_json::to_string(rows)).unwrap_or_else(|_| "[]".to_string())
}

/// [`operators_json`] as a [`Value`], for a host that converts one.
pub fn operators_value(engine: &Engine) -> Value {
    with_operators(engine, |rows| serde_json::to_value(rows))
        .unwrap_or_else(|_| Value::Array(Vec::new()))
}

fn with_operators<T>(engine: &Engine, f: impl FnOnce(&[OperatorJson<'_>]) -> T) -> T {
    let ops: Vec<OperatorInfo> = engine.operators().collect();
    let rows: Vec<OperatorJson<'_>> = ops.iter().map(OperatorJson::from).collect();
    f(&rows)
}

/// A rule's [`Facts`] as JSON. `reads` lists each path as its segments
/// (`[["user", "id"], ["items"]]`), which keeps a key containing a dot
/// unambiguous.
pub fn facts_json(facts: &Facts) -> String {
    serde_json::to_string(&facts_wire(facts)).unwrap_or_default()
}

/// [`facts_json`] as a [`Value`], for a host that converts one.
pub fn facts_value(facts: &Facts) -> Value {
    serde_json::to_value(facts_wire(facts)).unwrap_or(Value::Null)
}

fn facts_wire(facts: &Facts) -> impl Serialize + '_ {
    #[derive(Serialize)]
    struct Wire<'a> {
        reads: Vec<&'a [String]>,
        computed_reads: bool,
        reads_complete: bool,
        reads_data: bool,
        operators: &'a [&'static str],
        custom_operators: &'a [String],
        deterministic: bool,
    }
    Wire {
        reads: facts.reads().iter().map(|p| p.segments()).collect(),
        computed_reads: facts.has_computed_reads(),
        reads_complete: facts.reads_complete(),
        reads_data: facts.reads_data(),
        operators: facts.operators(),
        custom_operators: facts.custom_operators(),
        deterministic: facts.is_deterministic(),
    }
}

/// Diagnostics from [`Engine::check`] as a JSON array.
pub fn diagnostics_json(diagnostics: &[Diagnostic]) -> String {
    serde_json::to_string(diagnostics).unwrap_or_else(|_| "[]".to_string())
}

/// [`diagnostics_json`] as a [`Value`], for a host that converts one.
pub fn diagnostics_value(diagnostics: &[Diagnostic]) -> Value {
    serde_json::to_value(diagnostics).unwrap_or_else(|_| Value::Array(Vec::new()))
}

/// The [`Family`] a host names, by its catalogue name (`"ExtString"`,
/// `"DateTime"`, ...: the `family` of each [`operators_json`] row).
pub fn family(name: &str) -> Result<Family, String> {
    Family::ALL
        .iter()
        .copied()
        .find(|f| f.name() == name)
        .ok_or_else(|| {
            let known: Vec<&str> = Family::ALL.iter().map(|f| f.name()).collect();
            format!(
                "unknown operator family {name:?} (expected one of: {})",
                known.join(", ")
            )
        })
}

/// [`family`] for each name in `names`.
pub fn families<'a>(names: impl IntoIterator<Item = &'a str>) -> Result<Vec<Family>, String> {
    names.into_iter().map(family).collect()
}

/// [`families`] from a JSON array of names, the form the C ABI takes.
pub fn families_from_json(json: &str) -> Result<Vec<Family>, String> {
    let names: Vec<String> = serde_json::from_str(json)
        .map_err(|_| "families must be a JSON array of family names".to_string())?;
    families(names.iter().map(String::as_str))
}

/// The [`CheckMode`] a host names: `"engine"` (or none), `"strict"`,
/// `"template"`.
pub fn check_mode(name: Option<&str>) -> Result<CheckMode, String> {
    match name {
        None | Some("engine") => Ok(CheckMode::Engine),
        Some("strict") => Ok(CheckMode::Strict),
        Some("template") => Ok(CheckMode::Template),
        Some(other) => Err(format!(
            "unknown check mode {other:?} (expected \"engine\", \"strict\" or \"template\")"
        )),
    }
}

/// Compile `rule` the way `mode` reads it: [`Engine::compile`] for
/// [`CheckMode::Engine`], [`Engine::compile_strict`],
/// [`Engine::compile_template`].
pub fn compile_in<R: IntoLogic>(engine: &Engine, rule: R, mode: CheckMode) -> Result<Logic, Error> {
    match mode {
        CheckMode::Strict => engine.compile_strict(rule),
        CheckMode::Template => engine.compile_template(rule),
        _ => engine.compile(rule),
    }
}

/// The one character of a template key escape a host passed, or `None`
/// when it is empty or longer; each binding words its own refusal.
pub fn single_char(s: &str) -> Option<char> {
    let mut chars = s.chars();
    match (chars.next(), chars.next()) {
        (Some(c), None) => Some(c),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_catalogue_is_the_docs_file() {
        let engine = Engine::new();
        // Keys in catalogue order, so hosts that compare ordered maps agree.
        let first = operators_json(&engine);
        assert!(
            first.starts_with(r#"[{"name":"val","aliases":["var"],"family":"Core""#),
            "{first}"
        );
        let got: Value = serde_json::from_str(&first).unwrap();
        let path = concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../docs/src/operators/operators.json"
        );
        let docs: Value = serde_json::from_str(&std::fs::read_to_string(path).unwrap()).unwrap();
        assert_eq!(got, docs);
    }

    #[test]
    fn facts_have_their_fields() {
        let engine = Engine::new();
        let logic = engine
            .compile(r#"{"+": [{"var": "a.b"}, {"var": "c"}]}"#)
            .unwrap();
        let v: Value = serde_json::from_str(&facts_json(&logic.facts())).unwrap();
        assert_eq!(v["reads"], json!([["a", "b"], ["c"]]));
        assert_eq!(v["operators"], json!(["+", "val"]));
        assert_eq!(v["reads_complete"], true);
        assert_eq!(v["deterministic"], true);
        assert_eq!(v["computed_reads"], false);
    }

    #[test]
    fn diagnostics_serialise() {
        let engine = Engine::new();
        let d = engine.check(r#"{"if": [true, {"vr": 1}]}"#, CheckMode::Engine);
        let v: Value = serde_json::from_str(&diagnostics_json(&d)).unwrap();
        assert_eq!(v[0]["code"], "UnknownOperator");
        assert_eq!(v[0]["pointer"], "/if/1");
    }

    #[test]
    fn budgets() {
        assert_eq!(budget_from_f64(None), Ok(None));
        assert_eq!(budget_from_f64(Some(5.0)), Ok(Some(5)));
        for bad in [
            0.0,
            -1.0,
            1.5,
            f64::NAN,
            f64::INFINITY,
            MAX_SAFE_BUDGET + 2.0,
        ] {
            assert_eq!(budget_from_f64(Some(bad)), Err(BUDGET_ERROR), "{bad}");
        }
    }

    #[test]
    fn resolved_budgets() {
        let engine = Engine::new();
        assert_eq!(
            resolve_budget(&engine, None),
            Ok(engine.resolve_ops_budget(None))
        );
        assert_eq!(resolve_budget(&engine, Some(7.0)), Ok(7));
        assert_eq!(resolve_budget(&engine, Some(0.5)), Err(BUDGET_ERROR));
    }

    #[test]
    fn family_names() {
        assert_eq!(family("ExtString"), Ok(Family::ExtString));
        assert_eq!(family("Core"), Ok(Family::Core));
        let err = family("Strings").unwrap_err();
        assert!(
            err.contains("\"Strings\"") && err.contains("ExtString"),
            "{err}"
        );
        assert_eq!(
            families_from_json(r#"["DateTime", "ExtArray"]"#),
            Ok(vec![Family::DateTime, Family::ExtArray])
        );
        assert_eq!(families_from_json("[]"), Ok(vec![]));
        assert!(families_from_json(r#"["Nope"]"#).is_err());
        assert!(families_from_json(r#"{"a": 1}"#).is_err());
        // Every catalogue family name parses.
        let engine = Engine::new();
        for op in engine.operators() {
            assert!(family(op.family).is_ok(), "{}", op.family);
        }
    }

    #[test]
    fn modes() {
        assert_eq!(check_mode(None), Ok(CheckMode::Engine));
        assert_eq!(check_mode(Some("template")), Ok(CheckMode::Template));
        assert!(check_mode(Some("loose")).is_err());
    }

    #[test]
    fn custom_operator_round_trip() {
        let arena = Bump::new();
        let a = DataValue::from_str("1", &arena).unwrap();
        let b = DataValue::from_str(r#""x""#, &arena).unwrap();
        assert_eq!(args_json(&[&a, &b]), r#"[1,"x"]"#);
        assert_eq!(
            parse_result("op", "[1,2]", &arena)
                .unwrap()
                .to_json_string(),
            "[1,2]"
        );
        let err = parse_result("op", "nope", &arena).unwrap_err();
        assert!(
            err.to_string()
                .contains("custom operator 'op' returned invalid JSON")
        );
    }

    #[test]
    fn traced_json_places_every_node() {
        let engine = Engine::new();
        let rule = r#"{"if": [{"var": "a"}, {"+": [1, 2]}, "no"]}"#;
        let v: Value = serde_json::from_str(&traced_json(&engine, rule, r#"{"a": true}"#)).unwrap();
        assert_eq!(v["result"], 3);
        let pointers = v["pointers"].as_object().unwrap();
        let root = v["expression_tree"]["id"].to_string();
        assert_eq!(pointers[&root], "");
        let rule: Value = serde_json::from_str(rule).unwrap();
        for step in v["steps"].as_array().unwrap() {
            let p = pointers[&step["node_id"].to_string()].as_str().unwrap();
            assert!(rule.pointer(p).is_some(), "{p}");
        }
        // The same envelope as `traced_run_json` apart from `pointers`.
        let plain: Value = serde_json::from_str(&traced_run_json(
            &engine.trace().eval_str(&rule, r#"{"a": true}"#),
        ))
        .unwrap();
        let mut without = v.clone();
        without.as_object_mut().unwrap().remove("pointers");
        assert_eq!(without, plain);
    }

    #[test]
    fn traced_json_reports_a_compile_failure() {
        let engine = Engine::new();
        let v: Value =
            serde_json::from_str(&traced_json(&engine, r#"{"a": 1, "b": 2}"#, "null")).unwrap();
        assert!(v.get("pointers").is_none());
        assert!(v.get("error").is_some());
        let plain: Value = serde_json::from_str(&traced_run_json(
            &engine.trace().eval_str(r#"{"a": 1, "b": 2}"#, "null"),
        ))
        .unwrap();
        assert_eq!(v, plain);
    }

    #[test]
    fn traced_runs() {
        let engine = Engine::new();
        let run = engine
            .trace()
            .eval_str(r#"{"+": [1, {"var": "x"}]}"#, r#"{"x": 2}"#);
        let v: Value = serde_json::from_str(&traced_run_json(&run)).unwrap();
        assert_eq!(v["result"], 3);
        assert!(v.get("error").is_none());
        let run = engine.trace().eval_str(r#"{"+": ["a", 1]}"#, "null");
        let v: Value = serde_json::from_str(&traced_run_json(&run)).unwrap();
        assert_eq!(v["result"], Value::Null);
        assert_eq!(v["structured_error"]["type"], "Thrown");
    }
}
