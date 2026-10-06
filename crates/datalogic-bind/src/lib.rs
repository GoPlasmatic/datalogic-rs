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
//! | Batch item failure: `{tag, message, operator?}` | [`ItemError`] |
//! | Error path: `[{node_id, operator, arg_index, json_pointer}]` (or camelCase) | [`path_value`] |
//!
//! The plumbing: [`EngineOptions`] and [`add_operators`] assemble an engine
//! from constructor options, [`typed`] reads typed results, [`same_engine`]
//! checks a session's rule, and [`resolve_budget`] / [`resolve_budget_u64`]
//! settle an operation budget.

use datalogic_rs::bumpalo::Bump;
use std::collections::BTreeMap;
use std::sync::Arc;

use datalogic_rs::{
    CheckMode, CustomOperator, DataValue, Diagnostic, Engine, EngineBuilder, Error,
    EvaluationConfig, ExecutionStep, ExpressionNode, Facts, Family, IntoLogic, Logic, OperatorInfo,
    ScopedArg, TracedRun,
};
use serde::Serialize;
use serde_json::value::RawValue;
use serde_json::{Value, json};

/// The JSON type name of a value, as the typed-result entry points report
/// a mismatch (`"result is not a boolean (got string)"`).
///
/// A value with no JSON type of its own (a datetime, a duration, a tensor)
/// is named by the JSON it serialises to: datetimes and durations cross
/// every boundary as strings, so they are `"string"` here too.
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
    } else if v.is_object() {
        "object"
    } else {
        // Only reached on a type-mismatch error path, so the serialisation
        // costs nothing on the success path.
        match v.to_json_string().as_bytes().first() {
            Some(b'"') => "string",
            Some(b'[') => "array",
            Some(b'n') => "null",
            Some(b't' | b'f') => "boolean",
            Some(b'{') | None => "object",
            Some(_) => "number",
        }
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
    traced_json_in(engine, rule, data, CheckMode::Engine)
}

/// [`traced_json`] with `rule` compiled the way `mode` reads it, as
/// [`compile_in`] compiles it, so a rule a host compiles as a template is
/// traced as one.
pub fn traced_json_in(engine: &Engine, rule: &str, data: &str, mode: CheckMode) -> String {
    let tracer = engine.trace().with_mode(mode);
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
    /// The run's result: the engine's own JSON text, spliced in as it is
    /// so an object keeps its key order, or the text as a JSON string in
    /// the unexpected case that it is not JSON.
    #[derive(Serialize)]
    #[serde(untagged)]
    enum ResultWire<'a> {
        Json(&'a RawValue),
        Text(&'a str),
    }

    #[derive(Serialize)]
    struct Wire<'a> {
        result: ResultWire<'a>,
        expression_tree: &'a ExpressionNode,
        steps: &'a [ExecutionStep],
        #[serde(skip_serializing_if = "Option::is_none")]
        error: Option<String>,
        #[serde(skip_serializing_if = "Option::is_none")]
        structured_error: Option<&'a Error>,
        #[serde(skip_serializing_if = "Option::is_none")]
        pointers: Option<BTreeMap<u32, &'a str>>,
    }

    let null: &RawValue = serde_json::from_str("null").expect("null is JSON");
    let (result, error, structured_error) = match &run.result {
        Ok(s) => (
            serde_json::from_str::<&RawValue>(s)
                .map(ResultWire::Json)
                .unwrap_or(ResultWire::Text(s)),
            None,
            None,
        ),
        Err(e) => (ResultWire::Json(null), Some(e.to_string()), Some(e)),
    };
    serde_json::to_string(&Wire {
        result,
        expression_tree: &run.expression_tree,
        steps: &run.steps,
        error,
        structured_error,
        pointers: logic.map(|logic| logic.pointers().collect()),
    })
    .unwrap_or_else(|e| {
        // Not expected: every field serialises. Still answer the envelope
        // a host parses, rather than text that is not JSON.
        json!({
            "result": null,
            "expression_tree": {"id": 0, "expression": "", "children": []},
            "steps": [],
            "error": format!("the trace could not be serialised: {e}"),
        })
        .to_string()
    })
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

/// The operation budget for one call from a host whose integers are exact
/// (Python): `budget` as given, or the engine's own budget when the host
/// passed none. Zero is refused with [`BUDGET_ERROR`], as the JavaScript
/// bindings refuse it, rather than meaning a budget nothing fits in: the
/// C-ABI bindings, which have no "none", spell the engine's budget `0`.
pub fn resolve_budget_u64(engine: &Engine, budget: Option<u64>) -> Result<u64, &'static str> {
    match budget {
        Some(0) => Err(BUDGET_ERROR),
        explicit => Ok(engine.resolve_ops_budget(explicit)),
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
    let mut json = Vec::with_capacity(64);
    write_args_json(args, &mut json);
    // The emitter writes UTF-8 only.
    String::from_utf8(json).unwrap_or_else(|e| String::from_utf8_lossy(e.as_bytes()).into_owned())
}

/// [`args_json`] appended to `out` as bytes, for a host that hands the
/// callback a byte range (the C ABI).
pub fn write_args_json(args: &[&DataValue<'_>], out: &mut Vec<u8>) {
    out.push(b'[');
    for (i, a) in args.iter().enumerate() {
        if i > 0 {
            out.push(b',');
        }
        a.write_json_into(out);
    }
    out.push(b']');
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

/// The engine options the Node, Python and WASM constructors take, once
/// each binding has read them from its host's values (and refused the bad
/// ones in its own words).
#[derive(Default)]
pub struct EngineOptions {
    /// Multi-key objects compile to output templates.
    pub templating: bool,
    /// The operator families besides the core; `None` keeps every family.
    pub families: Option<Vec<Family>>,
    /// The template-key escape character.
    pub template_key_escape: Option<char>,
    /// The evaluation configuration; `None` keeps the default.
    pub config: Option<EvaluationConfig>,
}

impl EngineOptions {
    /// A builder carrying these options, ready for custom operators.
    pub fn builder(self) -> EngineBuilder {
        let mut builder = Engine::builder().with_templating(self.templating);
        // Before the operators are added, so strict names are judged
        // against the families and escape the engine has.
        if let Some(families) = self.families {
            builder = builder.with_families(families);
        }
        if let Some(c) = self.template_key_escape {
            builder = builder.with_template_key_escape(c);
        }
        if let Some(config) = self.config {
            builder = builder.with_config(config);
        }
        builder
    }
}

/// Register each of `operators` on `builder`, in order. With `strict`, a
/// name a built-in answers to is refused
/// ([`EngineBuilder::try_add_operator`]) with the first such name's
/// `ConfigurationError`.
pub fn add_operators<O: CustomOperator + 'static>(
    mut builder: EngineBuilder,
    operators: impl IntoIterator<Item = (String, O)>,
    strict: bool,
) -> Result<EngineBuilder, Error> {
    for (name, op) in operators {
        builder = if strict {
            builder.try_add_operator(name, op)?
        } else {
            builder.add_operator(name, op)
        };
    }
    Ok(builder)
}

/// The refusal a session gives a rule compiled by another engine, in the
/// bindings that check (the C ABI and Python's handle-based entry points).
pub const DIFFERENT_ENGINE: &str = "rule was compiled by a different engine than this session's";

/// Whether a rule compiled by `rule_engine` may run in a session of
/// `session_engine`: the same engine, or [`DIFFERENT_ENGINE`].
pub fn same_engine(
    session_engine: &Arc<Engine>,
    rule_engine: &Arc<Engine>,
) -> Result<(), &'static str> {
    if Arc::ptr_eq(session_engine, rule_engine) {
        Ok(())
    } else {
        Err(DIFFERENT_ENGINE)
    }
}

/// Typed results: a result read as one JSON type, or the mismatch message
/// the binding raises with its own `TypeMismatch` error.
pub mod typed {
    use super::{DataValue, type_of};

    /// A strict JSON boolean.
    pub fn bool(v: &DataValue<'_>) -> Result<bool, String> {
        v.as_bool()
            .ok_or_else(|| format!("result is not a boolean (got {})", type_of(v)))
    }

    /// Any JSON number, as a double.
    pub fn float(v: &DataValue<'_>) -> Result<f64, String> {
        v.as_f64()
            .ok_or_else(|| format!("result is not a number (got {})", type_of(v)))
    }

    /// An exact 64-bit integer (the C ABI and Python).
    pub fn int(v: &DataValue<'_>) -> Result<i64, String> {
        v.as_i64()
            .ok_or_else(|| format!("result is not an integer number (got {})", type_of(v)))
    }

    /// An integer a JavaScript number holds exactly, `|n| <= 2^53 - 1`
    /// (Node and WASM).
    pub fn safe_int(v: &DataValue<'_>) -> Result<f64, String> {
        match v.as_i64() {
            Some(i) if i.unsigned_abs() < (1u64 << 53) => Ok(i as f64),
            _ => Err(format!("result is not a safe integer (got {})", type_of(v))),
        }
    }
}

/// The key spelling of an error path's steps: the C ABI's JSON uses
/// `node_id`, Node's objects `nodeId`.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum PathKeys {
    /// `node_id`, `operator`, `arg_index`, `json_pointer`.
    Snake,
    /// `nodeId`, `operator`, `argIndex`, `jsonPointer`.
    Camel,
}

/// `err`'s node-id breadcrumb resolved against the rule it ran in, root to
/// leaf, as an array of step objects keyed as `keys` says.
pub fn path_value(err: &Error, compiled: &Logic, keys: PathKeys) -> Value {
    let (node_id, arg_index, json_pointer) = match keys {
        PathKeys::Snake => ("node_id", "arg_index", "json_pointer"),
        PathKeys::Camel => ("nodeId", "argIndex", "jsonPointer"),
    };
    Value::Array(
        err.resolve_path(compiled)
            .into_iter()
            .map(|s| {
                let mut step = serde_json::Map::new();
                step.insert(node_id.to_string(), json!(s.node_id));
                step.insert("operator".to_string(), json!(s.operator));
                step.insert(arg_index.to_string(), json!(s.arg_index));
                step.insert(json_pointer.to_string(), json!(s.json_pointer));
                Value::Object(step)
            })
            .collect(),
    )
}

/// One failed item of a batch call (one rule over many data, or many rules
/// over one data): `{tag, message, operator?}`, the fields every binding's
/// batch result carries for a failure.
#[derive(Serialize, Clone, Debug, PartialEq, Eq)]
pub struct ItemError {
    /// The engine's stable error tag, or `"InvalidArgument"` for a bad
    /// handle or rule in the batch.
    pub tag: String,
    /// The error's message.
    pub message: String,
    /// The outermost failing operator, when known.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub operator: Option<String>,
}

impl ItemError {
    /// The item failure for an engine error.
    pub fn from_engine(err: &Error) -> Self {
        Self {
            tag: err.tag().to_string(),
            message: err.to_string(),
            operator: err.operator().map(str::to_owned),
        }
    }

    /// The item failure for a bad argument in the batch.
    pub fn invalid_argument(message: impl Into<String>) -> Self {
        Self {
            tag: "InvalidArgument".to_string(),
            message: message.into(),
            operator: None,
        }
    }

    /// The failure as a JSON object value, keys in a [`Value`]'s own
    /// (sorted) order: what the C ABI writes and Node hands back.
    pub fn to_value(&self) -> Value {
        match &self.operator {
            Some(op) => json!({"tag": self.tag, "message": self.message, "operator": op}),
            None => json!({"tag": self.tag, "message": self.message}),
        }
    }
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
    // The wire holds only strings, lists and booleans, so this does not
    // fail; if it did, `null` is what `facts_value` answers too.
    serde_json::to_string(&facts_wire(facts)).unwrap_or_else(|_| "null".to_string())
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
    fn a_traced_result_keeps_its_key_order() {
        let engine = Engine::new();
        let data = r#"{"o": {"z": 1, "a": {"y": 2, "b": 3}}}"#;
        let text = traced_json(&engine, r#"{"var": "o"}"#, data);
        assert!(
            text.starts_with(r#"{"result":{"z":1,"a":{"y":2,"b":3}},"#),
            "{text}"
        );
        let run = engine.trace().eval_str(r#"{"var": "o"}"#, data);
        let text = traced_run_json(&run);
        assert!(text.starts_with(r#"{"result":{"z":1,"a":{"y":2,"b":3}},"#));
        // A failure still answers `null`.
        let text = traced_json(&engine, r#"{"throw": "x"}"#, "null");
        assert!(text.starts_with(r#"{"result":null,"#), "{text}");
    }

    #[test]
    fn type_names() {
        let arena = Bump::new();
        for (json, want) in [
            ("null", "null"),
            ("true", "boolean"),
            ("1.5", "number"),
            (r#""s""#, "string"),
            ("[1]", "array"),
            (r#"{"a": 1}"#, "object"),
        ] {
            let v = DataValue::from_str(json, &arena).unwrap();
            assert_eq!(type_of(&v), want, "{json}");
        }
        // A datetime or a duration is no JSON type of its own; each crosses
        // as a string. (The dev-dependencies turn the datetime family on.)
        use datalogic_rs::datavalue::{DataDateTime, DataDuration};
        let dt = DataValue::DateTime(DataDateTime::parse("2024-01-02T03:04:05Z").unwrap());
        assert!(!dt.is_string() && !dt.is_object());
        assert_eq!(type_of(&dt), "string");
        let du = DataValue::Duration(DataDuration::parse("1d:2h:3m:4s").unwrap());
        assert_eq!(type_of(&du), "string");
    }

    #[test]
    fn typed_results() {
        let arena = Bump::new();
        let v = |json: &'static str| DataValue::from_str(json, &arena).unwrap();
        assert_eq!(typed::bool(&v("true")), Ok(true));
        assert_eq!(
            typed::bool(&v("1")),
            Err("result is not a boolean (got number)".to_string())
        );
        assert_eq!(typed::float(&v("2")), Ok(2.0));
        assert_eq!(
            typed::float(&v(r#""2""#)),
            Err("result is not a number (got string)".to_string())
        );
        assert_eq!(typed::int(&v("7")), Ok(7));
        assert_eq!(
            typed::int(&v("7.5")),
            Err("result is not an integer number (got number)".to_string())
        );
        assert_eq!(
            typed::safe_int(&v("9007199254740991")),
            Ok(9007199254740991.0)
        );
        assert_eq!(
            typed::safe_int(&v("9007199254740992")),
            Err("result is not a safe integer (got number)".to_string())
        );
    }

    #[test]
    fn item_errors() {
        let engine = Engine::new();
        let err = engine.eval_str(r#"{"+": ["a", 1]}"#, "null").unwrap_err();
        let item = ItemError::from_engine(&err);
        assert_eq!(item.tag, "Thrown");
        assert_eq!(item.operator.as_deref(), Some("+"));
        assert_eq!(
            serde_json::to_string(&item).unwrap(),
            format!(
                r#"{{"tag":"Thrown","message":{},"operator":"+"}}"#,
                json!(item.message)
            )
        );
        assert_eq!(
            item.to_value().to_string(),
            format!(
                r#"{{"message":{},"operator":"+","tag":"Thrown"}}"#,
                json!(item.message)
            )
        );
        let bad = ItemError::invalid_argument("data handle is null");
        assert_eq!(
            serde_json::to_string(&bad).unwrap(),
            r#"{"tag":"InvalidArgument","message":"data handle is null"}"#
        );
        assert_eq!(
            bad.to_value().to_string(),
            r#"{"message":"data handle is null","tag":"InvalidArgument"}"#
        );
    }

    #[test]
    fn error_paths() {
        let engine = Engine::new();
        let logic = engine
            .compile(r#"{"if": [{"var": "c"}, {"+": [{"var": "a"}, 1]}]}"#)
            .unwrap();
        let arena = Bump::new();
        let err = engine
            .evaluate(&logic, r#"{"c": true, "a": "x"}"#, &arena)
            .unwrap_err();
        let snake = path_value(&err, &logic, PathKeys::Snake);
        let plus = |steps: &Value, key: &str| {
            steps
                .as_array()
                .unwrap()
                .iter()
                .find(|s| s["operator"] == "+")
                .map(|s| s[key].clone())
        };
        assert_eq!(plus(&snake, "json_pointer"), Some(json!("/if/1")));
        assert!(plus(&snake, "node_id").is_some_and(|v| v.is_number()));
        let camel = path_value(&err, &logic, PathKeys::Camel);
        assert_eq!(plus(&camel, "jsonPointer"), Some(json!("/if/1")));
        assert_eq!(plus(&camel, "argIndex"), Some(json!(1)));
    }

    #[test]
    fn engine_options() {
        let engine = EngineOptions {
            templating: true,
            families: Some(vec![]),
            template_key_escape: Some('$'),
            config: Some(EvaluationConfig::from_json_str(r#"{"missing_var": "error"}"#).unwrap()),
        }
        .builder()
        .build();
        assert_eq!(
            engine.eval_str(r#"{"$type": 1, "k": 2}"#, "null").unwrap(),
            r#"{"type":1,"k":2}"#
        );
        assert!(engine.eval_str(r#"{"var": "nope"}"#, "{}").is_err());
        assert_eq!(
            engine.check(r#"{"length": "abc"}"#, CheckMode::Strict)[0].code,
            datalogic_rs::DiagnosticCode::UnknownOperator
        );

        struct Nop;
        impl CustomOperator for Nop {
            fn evaluate<'a>(
                &self,
                _args: &[&'a DataValue<'a>],
                _ctx: &mut datalogic_rs::operator::EvalContext<'_, 'a>,
                arena: &'a Bump,
            ) -> datalogic_rs::Result<&'a DataValue<'a>> {
                Ok(arena.alloc(DataValue::Null))
            }
        }
        let strict = add_operators(
            EngineOptions::default().builder(),
            [("length".to_string(), Nop)],
            true,
        );
        assert_eq!(strict.err().map(|e| e.tag()), Some("ConfigurationError"));
        // Not strict, or once the family is left out, the name is free.
        assert!(
            add_operators(
                EngineOptions::default().builder(),
                [("length".to_string(), Nop)],
                false
            )
            .is_ok()
        );
        let freed = EngineOptions {
            families: Some(vec![]),
            ..Default::default()
        };
        assert!(add_operators(freed.builder(), [("length".to_string(), Nop)], true).is_ok());
    }

    #[test]
    fn engines() {
        let a = Arc::new(Engine::new());
        let b = Arc::new(Engine::new());
        assert_eq!(same_engine(&a, &a.clone()), Ok(()));
        assert_eq!(same_engine(&a, &b), Err(DIFFERENT_ENGINE));
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
