//! `Engine` and `Rule` pyclasses — the heart of the binding.

use std::collections::HashMap;
use std::sync::Arc;

use datalogic_rs::bumpalo::Bump;
use datalogic_rs::operator::EvalContext;
use datalogic_rs::{
    CheckMode, CustomOperator, DataValue, Engine as RsEngine, Error as DlError, EvaluationConfig,
    Logic, Result as DlResult,
};
use pyo3::prelude::*;
use pyo3::types::{PyAny, PyString};
use serde_json::Value;

use crate::conv::{datavalue_to_pyobject, dict_to_value, value_to_pyobject};
use crate::data::{DataHandle, build_py_tree};
use crate::error::{engine_error_to_pyerr, evaluate_error_with_type};
use crate::session::Session;

/// JSONLogic compile/evaluate engine.
///
/// Construct once at startup and share across threads — `Engine` is
/// internally `Arc<datalogic_rs::Engine>` and Python's reference semantics
/// mean every reference points at the same underlying engine.
///
/// # Custom operators
///
/// Pass ``custom_operators={"name": callable, ...}`` to register custom
/// JSONLogic operators. Each callable receives the evaluated args as a
/// JSON-array string and must return a JSON string of the result::
///
///     engine = Engine(custom_operators={
///         "double": lambda args_json: json.dumps(json.loads(args_json)[0] * 2),
///     })
///     engine.eval_str('{"double": [21]}', '{}')  # "42"
///
/// Callbacks run while the GIL is held; the binding re-acquires the GIL
/// inside each operator call even if the surrounding evaluation released
/// it. **Built-ins win over custom registrations of the same name** — the
/// engine's built-in dispatcher never reaches a custom op with a name like
/// ``"+"`` or ``"if"``.
#[pyclass(name = "Engine", module = "datalogic_py", frozen)]
pub struct Engine {
    pub(crate) inner: Arc<RsEngine>,
}

#[pymethods]
impl Engine {
    /// Create a new engine.
    ///
    /// :param templating: when ``True``, multi-key objects in compiled
    ///     rules become output-shaping templates (the engine's "templating
    ///     mode"). Off by default.
    /// :param custom_operators: optional dict ``{name: callable}`` whose
    ///     values are ``Callable[[str], str]`` — JSON-array string in,
    ///     JSON value string out.
    /// :param config: optional evaluation configuration, as a ``dict`` or
    ///     a JSON ``str``. Accepts an optional ``"preset"`` key
    ///     (``"default"``, ``"safe_arithmetic"``, or ``"strict"``) plus
    ///     per-field overrides: ``arithmetic_nan_handling``,
    ///     ``division_by_zero``, ``loose_equality_errors``,
    ///     ``truthy_evaluator``, ``numeric_coercion``, and
    ///     ``max_recursion_depth``. Unknown keys or values raise
    ///     :class:`EvaluateError` with the engine's message.
    /// :param template_key_escape: one character that marks a template key
    ///     as a literal output field: with ``"$"``, ``{"$type": ...}`` emits
    ///     the key ``type`` instead of calling the ``type`` operator. Only
    ///     meaningful with ``templating``.
    /// :param strict_operator_names: when ``True``, a custom operator named
    ///     like a built-in (``"length"``, ``"var"``, an alias such as
    ///     ``"?:"``) raises :class:`EvaluateError` (``error_type ==
    ///     "ConfigurationError"``) instead of being registered and never
    ///     running.
    /// :param families: the operator families the engine has besides the
    ///     JSONLogic core (``["ExtString", "DateTime"]``: the ``family`` of
    ///     each :meth:`operators` row). ``None`` means every family. A
    ///     family left out is not there for the engine: its names compile
    ///     as unknown operators, and a custom operator may take them. An
    ///     unknown family name raises :class:`EvaluateError`
    ///     (``error_type == "ConfigurationError"``).
    #[new]
    #[pyo3(signature = (*, templating = false, custom_operators = None, config = None, strict_operator_names = false, template_key_escape = None, families = None))]
    fn new(
        py: Python<'_>,
        templating: bool,
        custom_operators: Option<HashMap<String, Py<PyAny>>>,
        config: Option<&Bound<'_, PyAny>>,
        strict_operator_names: bool,
        template_key_escape: Option<&str>,
        families: Option<Vec<String>>,
    ) -> PyResult<Self> {
        let mut opts = datalogic_bind::EngineOptions {
            templating,
            ..Default::default()
        };
        if let Some(names) = families {
            let families =
                datalogic_bind::families(names.iter().map(String::as_str)).map_err(|msg| {
                    engine_error_to_pyerr(py, &DlError::configuration_error(msg), None)
                })?;
            opts.families = Some(families);
        }
        if let Some(prefix) = template_key_escape {
            let c = datalogic_bind::single_char(prefix).ok_or_else(|| {
                engine_error_to_pyerr(
                    py,
                    &DlError::invalid_arguments(
                        "template_key_escape must be exactly one character",
                    ),
                    None,
                )
            })?;
            opts.template_key_escape = Some(c);
        }
        if let Some(cfg) = config {
            // Accept a JSON string as-is; anything else (normally a dict)
            // is serialised to JSON first. Both forms funnel into the
            // core's shared config parser so every binding rejects the
            // same typos with the same messages.
            let json = if let Ok(s) = cfg.cast::<PyString>() {
                s.to_str()?.to_string()
            } else {
                dict_to_value(py, cfg)?.to_string()
            };
            let parsed = EvaluationConfig::from_json_str(&json)
                .map_err(|e| engine_error_to_pyerr(py, &e, None))?;
            opts.config = Some(parsed);
        }
        let operators = custom_operators
            .into_iter()
            .flatten()
            .map(|(name, callback)| {
                let op = PyOperator {
                    name: name.clone(),
                    callback,
                };
                (name, op)
            });
        let builder =
            datalogic_bind::add_operators(opts.builder(), operators, strict_operator_names)
                .map_err(|e| engine_error_to_pyerr(py, &e, None))?;
        Ok(Self {
            inner: Arc::new(builder.build()),
        })
    }

    /// Compile a JSONLogic rule into a reusable [`Rule`].
    ///
    /// :param rule: a Python ``dict``/``list``/scalar describing the rule,
    ///     or a ``str`` containing the rule as JSON.
    fn compile(&self, py: Python<'_>, rule: &Bound<'_, PyAny>) -> PyResult<Rule> {
        let logic = compile_inner(py, &self.inner, rule)?;
        Ok(self.rule(logic))
    }

    /// Compile ``rule`` in templating mode, whatever this engine was built
    /// with: a multi-key object is an output template and an unknown key
    /// an output field.
    fn compile_template(&self, py: Python<'_>, rule: &Bound<'_, PyAny>) -> PyResult<Rule> {
        self.compile_in(py, rule, CheckMode::Template)
    }

    /// Compile ``rule`` outside templating mode, whatever this engine was
    /// built with: a multi-key object or an unknown operator is an error.
    fn compile_strict(&self, py: Python<'_>, rule: &Bound<'_, PyAny>) -> PyResult<Rule> {
        self.compile_in(py, rule, CheckMode::Strict)
    }

    /// Compile ``rule``, refusing it if :meth:`check` finds any error.
    /// Raises :class:`CompileError`, whose ``.diagnostics`` lists them.
    fn compile_checked(&self, py: Python<'_>, rule: &Bound<'_, PyAny>) -> PyResult<Rule> {
        let logic = with_rule(py, rule, |r| match r {
            RuleSrc::Text(s) => self.inner.compile_checked(s),
            RuleSrc::Json(v) => self.inner.compile_checked(v),
        })?
        .map_err(|e| crate::error::compile_error_to_pyerr(py, &e))?;
        Ok(self.rule(Arc::new(logic)))
    }

    /// Every problem this engine can see in ``rule`` before it runs, as a
    /// list of ``{"code", "severity", "message", "pointer", "operator"}``
    /// dicts. ``mode`` is ``"engine"`` (default), ``"strict"`` or
    /// ``"template"``.
    #[pyo3(signature = (rule, mode = None))]
    fn check(
        &self,
        py: Python<'_>,
        rule: &Bound<'_, PyAny>,
        mode: Option<&str>,
    ) -> PyResult<Py<PyAny>> {
        let mode = datalogic_bind::check_mode(mode)
            .map_err(|msg| engine_error_to_pyerr(py, &DlError::invalid_arguments(msg), None))?;
        let diagnostics = with_rule(py, rule, |r| match r {
            RuleSrc::Text(s) => self.inner.check(s, mode),
            RuleSrc::Json(v) => self.inner.check(v, mode),
        })?;
        value_to_pyobject(py, &datalogic_bind::diagnostics_value(&diagnostics))
    }

    /// Every built-in operator this engine evaluates, as a list of dicts:
    /// ``name``, ``aliases``, ``family``, ``feature``, ``min_args``,
    /// ``max_args``, ``reads_context``, ``effect``, ``cost`` and
    /// ``scoped_arg``.
    fn operators(&self, py: Python<'_>) -> PyResult<Py<PyAny>> {
        value_to_pyobject(py, &datalogic_bind::operators_value(&self.inner))
    }

    /// Whether ``value`` is truthy under this engine's configured
    /// truthiness. Under the default rules an empty dict is falsy, like an
    /// empty list. A ``str`` is JSON text, as data is everywhere else here
    /// (and in the C-ABI bindings): ``truthy("[]")`` is ``False``, and
    /// ``truthy('"a"')`` asks about the string ``a``.
    fn truthy(&self, py: Python<'_>, value: &Bound<'_, PyAny>) -> PyResult<bool> {
        if let Ok(s) = value.cast::<PyString>() {
            let parsed = datalogic_rs::ParsedData::from_json(s.to_str()?)
                .map_err(|e| engine_error_to_pyerr(py, &e, None))?;
            return Ok(self.inner.truthy_of(&parsed));
        }
        let value = dict_to_value(py, value)?;
        Ok(self.inner.truthy_of(&value))
    }

    /// One-shot evaluation. Compiles ``rule`` against ``data`` and returns
    /// the result as a Python value (``dict``/``list``/scalar/``None``).
    ///
    /// For repeated evaluations of the same rule, prefer
    /// :meth:`compile` + :meth:`Rule.evaluate` — it skips re-parsing.
    fn eval(
        &self,
        py: Python<'_>,
        rule: &Bound<'_, PyAny>,
        data: &Bound<'_, PyAny>,
    ) -> PyResult<Py<PyAny>> {
        let logic = compile_inner(py, &self.inner, rule)?;
        evaluate_value(py, &self.inner, &logic, data)
    }

    /// One-shot evaluation returning the result as a JSON ``str``.
    fn eval_str(
        &self,
        py: Python<'_>,
        rule: &Bound<'_, PyAny>,
        data: &Bound<'_, PyAny>,
    ) -> PyResult<String> {
        let logic = compile_inner(py, &self.inner, rule)?;
        evaluate_str(py, &self.inner, &logic, data)
    }

    /// One-shot metered evaluation: compile ``rule``, evaluate it
    /// against ``data`` under an operation budget, and report what it
    /// cost.
    ///
    /// Returns ``(result_json, ops)``: the result as a JSON ``str``, and
    /// the number of operations charged.
    ///
    /// An operation is one dispatched node, one item examined by an
    /// iterator, or whatever an operator charges for the data it moves —
    /// the tensor family prices itself in elements. Literals and
    /// constant-folded subtrees cost nothing. The count is deterministic
    /// for a given rule, data and engine version; budget for the work you
    /// want to allow rather than for a number you measured.
    ///
    /// :param budget: ceiling on the operations the rule may charge, at
    ///     least 1 (``0`` raises ``EvaluateError`` with ``error_type ==
    ///     "InvalidArgument"``, as the JavaScript bindings refuse it).
    ///     ``None`` falls back to the engine's ``ops_budget`` config key,
    ///     and meters without bounding if that is unset.
    /// :raises DataLogicError: with ``error_type == "BudgetExceeded"``
    ///     (carrying ``budget`` and ``spent``) when the rule charges past
    ///     the ceiling. The evaluation is refused before the work, and a
    ///     ``try`` in the rule cannot recover from it.
    #[pyo3(signature = (rule, data, budget = None))]
    fn eval_metered(
        &self,
        py: Python<'_>,
        rule: &Bound<'_, PyAny>,
        data: &Bound<'_, PyAny>,
        budget: Option<u64>,
    ) -> PyResult<(String, u64)> {
        let logic = compile_inner(py, &self.inner, rule)?;
        evaluate_metered(py, &self.inner, &logic, data, budget)
    }

    /// Evaluate ``logic`` against ``data`` with step-by-step execution
    /// tracing. Both arguments are JSON ``str``.
    ///
    /// Returns a JSON ``str`` envelope of the form
    /// ``{"result", "expression_tree", "steps", "error"?, "structured_error"?}``,
    /// identical to the WASM binding's ``evaluateWithTrace`` so the React
    /// debugger UI can consume it directly. Runtime failures do not raise:
    /// the envelope's ``error`` (message string) and ``structured_error``
    /// (structured form) fields carry them instead, alongside the steps
    /// recorded up to the failure. ``mode`` is ``"engine"`` (default),
    /// ``"strict"`` or ``"template"``, as for :meth:`check`, so a rule
    /// compiled with :meth:`compile_template` is traced as one.
    #[pyo3(signature = (logic, data, mode = None))]
    fn evaluate_with_trace(
        &self,
        py: Python<'_>,
        logic: &str,
        data: &str,
        mode: Option<&str>,
    ) -> PyResult<String> {
        let mode = datalogic_bind::check_mode(mode)
            .map_err(|msg| engine_error_to_pyerr(py, &DlError::invalid_arguments(msg), None))?;
        let engine = self.inner.clone();
        let logic_owned = logic.to_string();
        let data_owned = data.to_string();
        Ok(py.detach(move || {
            datalogic_bind::traced_json_in(&engine, logic_owned.as_str(), data_owned.as_str(), mode)
        }))
    }

    /// Open a hot-loop [`Session`] bound to this engine. The session
    /// reuses one bumpalo arena across calls and is reset between
    /// evaluations to bound peak memory.
    ///
    /// Sessions are **not thread-safe** — open one per thread.
    fn session(&self) -> Session {
        Session::new(self.inner.clone())
    }

    fn __repr__(&self) -> String {
        "Engine()".to_string()
    }
}

/// A compiled JSONLogic rule.
///
/// Hold one and call :meth:`evaluate` against many data inputs without
/// re-parsing. ``Rule`` is thread-safe — share the same instance across
/// worker threads to evaluate in parallel; the binding releases the GIL
/// around each Rust evaluate call.
impl Engine {
    fn rule(&self, logic: Arc<Logic>) -> Rule {
        Rule {
            engine: self.inner.clone(),
            logic,
        }
    }

    /// Compile ``rule`` as ``mode`` reads it, whatever this engine was
    /// built with.
    fn compile_in(
        &self,
        py: Python<'_>,
        rule: &Bound<'_, PyAny>,
        mode: CheckMode,
    ) -> PyResult<Rule> {
        let logic = with_rule(py, rule, |r| match r {
            RuleSrc::Text(s) => datalogic_bind::compile_in(&self.inner, s, mode),
            RuleSrc::Json(v) => datalogic_bind::compile_in(&self.inner, v, mode),
        })?
        .map_err(|e| engine_error_to_pyerr(py, &e, None))?;
        Ok(self.rule(Arc::new(logic)))
    }
}

/// A rule as the caller passed it: JSON text, or a Python value.
enum RuleSrc<'a> {
    Text(&'a str),
    Json(&'a Value),
}

/// Run `f` (a compile or a check) over `rule` with the GIL released: the
/// rule is read into Rust first, and compiling a large rule must not stall
/// every other Python thread. A custom operator the compile calls
/// re-acquires the GIL itself.
fn with_rule<T: Send>(
    py: Python<'_>,
    rule: &Bound<'_, PyAny>,
    f: impl FnOnce(RuleSrc<'_>) -> T + Send,
) -> PyResult<T> {
    if let Ok(s) = rule.cast::<PyString>() {
        let text = s.to_str()?;
        return Ok(py.detach(|| f(RuleSrc::Text(text))));
    }
    let value = dict_to_value(py, rule)?;
    Ok(py.detach(|| f(RuleSrc::Json(&value))))
}

#[pyclass(name = "Rule", module = "datalogic_py", frozen)]
pub struct Rule {
    engine: Arc<RsEngine>,
    logic: Arc<Logic>,
}

impl Rule {
    pub(crate) fn logic(&self) -> &Arc<Logic> {
        &self.logic
    }

    pub(crate) fn engine_arc(&self) -> &Arc<RsEngine> {
        &self.engine
    }
}

#[pymethods]
impl Rule {
    /// What the rule reads and calls, from a walk of the compiled rule: a
    /// dict with ``reads`` (each root path as a list of segments),
    /// ``computed_reads``, ``reads_complete``, ``reads_data``,
    /// ``operators``, ``custom_operators`` and ``deterministic``.
    fn facts(&self, py: Python<'_>) -> PyResult<Py<PyAny>> {
        value_to_pyobject(py, &datalogic_bind::facts_value(&self.logic.facts()))
    }

    /// Evaluate against ``data`` and return the result as a Python value.
    ///
    /// :param data: a Python ``dict``/``list``/scalar, or a ``str``
    ///     containing the data as JSON. The dict path walks Python
    ///     objects straight into the engine's arena (faster than any
    ///     JSON round-trip); for a payload evaluated repeatedly, parse
    ///     it once into a :class:`DataHandle` and use
    ///     :meth:`evaluate_data`.
    fn evaluate(&self, py: Python<'_>, data: &Bound<'_, PyAny>) -> PyResult<Py<PyAny>> {
        evaluate_value(py, &self.engine, &self.logic, data)
    }

    /// Evaluate against ``data`` (a JSON ``str``) and return the result as
    /// a JSON ``str``. Skips dict ↔ value conversion entirely — the
    /// fastest string-shaped path through the binding.
    fn evaluate_str(&self, py: Python<'_>, data: &str) -> PyResult<String> {
        // Capture sendable references for the GIL-released closure.
        let engine: &RsEngine = &self.engine;
        let logic: &Logic = &self.logic;
        let result = py.detach(|| -> Result<String, datalogic_rs::Error> {
            let arena = Bump::new();
            let av = engine.evaluate(logic, data, &arena)?;
            Ok(av.to_string())
        });
        result.map_err(|e| engine_error_to_pyerr(py, &e, Some(&self.logic)))
    }

    /// Evaluate against ``data`` under an operation budget, returning
    /// ``(result_json, ops)``.
    ///
    /// Same metering as :meth:`Engine.eval_metered`, on an
    /// already-compiled rule. ``budget=None`` falls back to the engine's
    /// ``ops_budget`` config key.
    #[pyo3(signature = (data, budget = None))]
    fn evaluate_metered(
        &self,
        py: Python<'_>,
        data: &Bound<'_, PyAny>,
        budget: Option<u64>,
    ) -> PyResult<(String, u64)> {
        evaluate_metered(py, &self.engine, &self.logic, data, budget)
    }

    /// Evaluate against a pre-parsed :class:`DataHandle` and return the
    /// result as a Python value — zero parse work per call.
    ///
    /// Like :meth:`evaluate`, thread-safe: the handle is immutable and
    /// the binding releases the GIL around the Rust evaluate call.
    fn evaluate_data(&self, py: Python<'_>, data: &DataHandle) -> PyResult<Py<PyAny>> {
        let mut arena = Bump::new();
        let res = eval_borrowing(
            py,
            &self.engine,
            &self.logic,
            DetachInput::Tree(data.tree.value()),
            &mut arena,
        );
        match res {
            Ok(av) => datavalue_to_pyobject(py, av),
            Err(e) => Err(engine_error_to_pyerr(py, &e, Some(&self.logic))),
        }
    }

    /// Evaluate against a pre-parsed :class:`DataHandle` and return the
    /// result as a JSON ``str`` — the fastest repeated-payload path on
    /// `Rule`.
    fn evaluate_data_str(&self, py: Python<'_>, data: &DataHandle) -> PyResult<String> {
        let engine: &RsEngine = &self.engine;
        let logic: &Logic = &self.logic;
        let tree = &data.tree;
        py.detach(move || -> Result<String, datalogic_rs::Error> {
            let arena = Bump::new();
            let av = engine.evaluate(logic, tree.value(), &arena)?;
            Ok(av.to_string())
        })
        .map_err(|e| engine_error_to_pyerr(py, &e, Some(&self.logic)))
    }

    fn __repr__(&self) -> String {
        "Rule(<compiled>)".to_string()
    }
}

// =============== Custom operator bridge ===============

/// Custom operator backed by a Python callable. Args cross the boundary
/// as a JSON-array string; the return must be a JSON string.
struct PyOperator {
    name: String,
    callback: Py<PyAny>,
}

impl CustomOperator for PyOperator {
    fn evaluate<'a>(
        &self,
        args: &[&'a DataValue<'a>],
        _ctx: &mut EvalContext<'_, 'a>,
        arena: &'a Bump,
    ) -> DlResult<&'a DataValue<'a>> {
        // 1. Build the args JSON array.
        let json = datalogic_bind::args_json(args);

        // 2. Acquire the GIL and call the Python callable. `Python::attach`
        //    re-acquires the GIL even if the surrounding evaluation
        //    released it via `py.detach`.
        let result_str: String = Python::attach(|py| -> Result<String, DlError> {
            let callable = self.callback.bind(py);
            let ret = callable.call1((json.as_str(),)).map_err(|e| {
                DlError::custom_message(format!("custom operator '{}' raised: {}", self.name, e))
            })?;
            ret.extract::<String>().map_err(|e| {
                DlError::custom_message(format!(
                    "custom operator '{}' must return a JSON string: {}",
                    self.name, e
                ))
            })
        })?;

        // 3. Parse the returned JSON into the eval arena so the borrowed
        //    `DataValue` outlives this call.
        datalogic_bind::parse_result(&self.name, &result_str, arena)
    }
}

// ---------------- shared helpers ----------------

pub(crate) fn compile_inner(
    py: Python<'_>,
    engine: &Arc<RsEngine>,
    rule: &Bound<'_, PyAny>,
) -> PyResult<Arc<Logic>> {
    with_rule(py, rule, |r| match r {
        RuleSrc::Text(s) => engine.compile_arc(s),
        RuleSrc::Json(v) => engine.compile_arc(v),
    })?
    .map_err(|e| engine_error_to_pyerr(py, &e, None))
}

/// Input shapes for [`eval_borrowing`]. Both are `Send` references into
/// caller-owned storage that outlives the evaluation.
pub(crate) enum DetachInput<'a> {
    /// JSON text; parsed into the arena by the engine (zero-copy — the
    /// parsed strings may borrow the text, so it must outlive the
    /// result).
    Str(&'a str),
    /// An already-resident tree (`DataHandle` or the dict fast path's
    /// walked cell); passed through at zero cost.
    Tree(&'a DataValue<'a>),
}

/// Evaluate with the GIL released and hand back the **borrowed** arena
/// result, so the caller can convert it straight to Python objects (or
/// project a typed scalar) without materialising an intermediate tree.
///
/// The `&mut Bump` is what lets the borrow escape `py.detach`: the
/// closure owns the unique reference (`&mut Bump: Send` even though
/// `Bump: !Sync`) and downgrades it to `&'a Bump` for the whole borrow,
/// so the returned `&'a DataValue` stays valid until the caller drops
/// or resets the arena.
pub(crate) fn eval_borrowing<'a>(
    py: Python<'_>,
    engine: &RsEngine,
    // Shares `'a` because the core's `evaluate` may return references
    // into the compiled rule's pre-built literals — callers keep the
    // `Arc<Logic>` alive until the result is converted.
    logic: &'a Logic,
    input: DetachInput<'a>,
    arena: &'a mut Bump,
) -> Result<&'a DataValue<'a>, DlError> {
    py.detach(move || {
        let arena: &'a Bump = arena;
        match input {
            DetachInput::Str(s) => engine.evaluate(logic, s, arena),
            DetachInput::Tree(v) => engine.evaluate(logic, v, arena),
        }
    })
}

pub(crate) fn evaluate_value(
    py: Python<'_>,
    engine: &Arc<RsEngine>,
    logic: &Arc<Logic>,
    data: &Bound<'_, PyAny>,
) -> PyResult<Py<PyAny>> {
    // Fast path: if the caller already has a JSON string, skip dict
    // conversion — parse and evaluate in one detached pass, then convert
    // the borrowed result directly.
    if let Ok(s) = data.cast::<PyString>() {
        let text = s.to_str()?.to_string();
        let mut arena = Bump::new();
        let res = eval_borrowing(py, engine, logic, DetachInput::Str(&text), &mut arena);
        return match res {
            Ok(av) => datavalue_to_pyobject(py, av),
            Err(e) => Err(engine_error_to_pyerr(py, &e, Some(logic))),
        };
    }
    // Dict path: walk the Python objects straight into an arena tree.
    // Shapes the walk doesn't cover fall back to the pythonize path.
    match build_py_tree(data) {
        Ok(tree) => {
            let mut arena = Bump::new();
            let res = eval_borrowing(
                py,
                engine,
                logic,
                DetachInput::Tree(tree.value()),
                &mut arena,
            );
            match res {
                Ok(av) => datavalue_to_pyobject(py, av),
                Err(e) => Err(engine_error_to_pyerr(py, &e, Some(logic))),
            }
        }
        Err(_) => {
            let value = dict_to_value(py, data)?;
            let json = run_eval_to_value(py, engine, logic, &value)?;
            value_to_pyobject(py, &json)
        }
    }
}

/// String-result evaluation under the engine's own budget. The metered
/// body already runs every input tier; with no explicit budget it
/// resolves to exactly what `Engine::evaluate` would apply, so this is
/// that body minus the count.
pub(crate) fn evaluate_str(
    py: Python<'_>,
    engine: &Arc<RsEngine>,
    logic: &Arc<Logic>,
    data: &Bound<'_, PyAny>,
) -> PyResult<String> {
    evaluate_metered(py, engine, logic, data, None).map(|(json, _)| json)
}

/// Shared body for the `*_metered` methods: evaluate under `budget` and
/// hand back `(result_json, ops)`.
///
/// `budget` of `None` resolves through `Engine::resolve_ops_budget` — the
/// engine's configured `ops_budget`, then unbounded — the same precedence
/// the JS bindings use, so a rule metered from Python and from Node
/// reports the same number.
pub(crate) fn evaluate_metered(
    py: Python<'_>,
    engine: &Arc<RsEngine>,
    logic: &Arc<Logic>,
    data: &Bound<'_, PyAny>,
    budget: Option<u64>,
) -> PyResult<(String, u64)> {
    let budget = datalogic_bind::resolve_budget_u64(engine, budget)
        .map_err(|msg| evaluate_error_with_type(py, msg.to_string(), "InvalidArgument"))?;
    let engine_ref: &RsEngine = engine;
    let logic_ref: &Logic = logic;

    // String input parses straight into the arena; anything else walks
    // the Python object tree.
    if let Ok(s) = data.cast::<PyString>() {
        let s_owned = s.to_str()?.to_string();
        return py
            .detach(|| -> Result<(String, u64), datalogic_rs::Error> {
                let arena = Bump::new();
                let m = engine_ref.evaluate_metered(logic_ref, s_owned.as_str(), &arena, budget)?;
                Ok((m.value.to_string(), m.ops))
            })
            .map_err(|e| engine_error_to_pyerr(py, &e, Some(logic)));
    }
    match build_py_tree(data) {
        Ok(tree) => py
            .detach(move || -> Result<(String, u64), datalogic_rs::Error> {
                let arena = Bump::new();
                let m = engine_ref.evaluate_metered(logic_ref, tree.value(), &arena, budget)?;
                Ok((m.value.to_string(), m.ops))
            })
            .map_err(|e| engine_error_to_pyerr(py, &e, Some(logic))),
        Err(_) => {
            let value = dict_to_value(py, data)?;
            py.detach(|| -> Result<(String, u64), datalogic_rs::Error> {
                let arena = Bump::new();
                let m = engine_ref.evaluate_metered(logic_ref, &value, &arena, budget)?;
                Ok((m.value.to_string(), m.ops))
            })
            .map_err(|e| engine_error_to_pyerr(py, &e, Some(logic)))
        }
    }
}

/// pythonize-fallback evaluation: `serde_json::Value` in, owned
/// `serde_json::Value` out. Only reached for input shapes the direct
/// walk doesn't cover.
fn run_eval_to_value(
    py: Python<'_>,
    engine: &Arc<RsEngine>,
    logic: &Arc<Logic>,
    value: &Value,
) -> PyResult<Value> {
    let engine_ref: &RsEngine = engine;
    let logic_ref: &Logic = logic;
    py.detach(|| -> Result<Value, datalogic_rs::Error> {
        let arena = Bump::new();
        let av = engine_ref.evaluate(logic_ref, value, &arena)?;
        serde_json::to_value(av).map_err(datalogic_rs::Error::wrap)
    })
    .map_err(|e| engine_error_to_pyerr(py, &e, Some(logic)))
}
