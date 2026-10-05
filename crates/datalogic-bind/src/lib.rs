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
//! | Traced run: `{result, expression_tree, steps, error?, structured_error?}` | [`traced_run_json`] |
//! | Operator catalogue: the schema of `docs/src/operators/operators.json` | [`operators_json`] |
//! | Rule facts: `{reads, computed_reads, reads_complete, reads_data, operators, custom_operators, deterministic}` | [`facts_json`] |
//! | Diagnostics: `[{code, severity, message, pointer, operator}]` | [`diagnostics_json`] |
//! | Custom operator call: arguments as one JSON array, result as JSON | [`args_json`], [`parse_result`] |

use datalogic_rs::bumpalo::Bump;
use datalogic_rs::{
    CheckMode, DataValue, Diagnostic, Engine, Error, ExecutionStep, ExpressionNode, Facts,
    OperatorInfo, ScopedArg, TracedRun,
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
    #[derive(Serialize)]
    struct Wire<'a> {
        result: Value,
        expression_tree: &'a ExpressionNode,
        steps: &'a [ExecutionStep],
        #[serde(skip_serializing_if = "Option::is_none")]
        error: Option<String>,
        #[serde(skip_serializing_if = "Option::is_none")]
        structured_error: Option<&'a Error>,
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
    })
    .unwrap_or_default()
}

/// The largest operation budget a JavaScript number carries exactly.
pub const MAX_SAFE_BUDGET: f64 = 9_007_199_254_740_991.0;

/// The message for a budget that is not a whole number of operations.
pub const BUDGET_ERROR: &str = "budget must be a whole number of operations >= 1";

/// Validate a budget a JavaScript host passed as a number: `None` stays
/// `None` (the engine's own budget applies), a whole number from 1 to
/// [`MAX_SAFE_BUDGET`] is taken, anything else is [`BUDGET_ERROR`].
pub fn budget_from_f64(budget: Option<f64>) -> Result<Option<u64>, &'static str> {
    match budget {
        None => Ok(None),
        Some(n) if n.is_finite() && n >= 1.0 && n.fract() == 0.0 && n <= MAX_SAFE_BUDGET => {
            Ok(Some(n as u64))
        }
        Some(_) => Err(BUDGET_ERROR),
    }
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
pub struct OperatorJson<'a> {
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
    let ops: Vec<OperatorInfo> = engine.operators().collect();
    let rows: Vec<OperatorJson<'_>> = ops.iter().map(OperatorJson::from).collect();
    serde_json::to_string(&rows).unwrap_or_else(|_| "[]".to_string())
}

/// A rule's [`Facts`] as JSON. `reads` lists each path as its segments
/// (`[["user", "id"], ["items"]]`), which keeps a key containing a dot
/// unambiguous.
pub fn facts_json(facts: &Facts) -> String {
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
    serde_json::to_string(&Wire {
        reads: facts.reads().iter().map(|p| p.segments()).collect(),
        computed_reads: facts.has_computed_reads(),
        reads_complete: facts.reads_complete(),
        reads_data: facts.reads_data(),
        operators: facts.operators(),
        custom_operators: facts.custom_operators(),
        deterministic: facts.is_deterministic(),
    })
    .unwrap_or_default()
}

/// Diagnostics from [`Engine::check`] as a JSON array.
pub fn diagnostics_json(diagnostics: &[Diagnostic]) -> String {
    serde_json::to_string(diagnostics).unwrap_or_else(|_| "[]".to_string())
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
