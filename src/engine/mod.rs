#[cfg(feature = "compat")]
use serde_json::Value;
use std::collections::HashMap;
use std::sync::Arc;

use crate::config::EvaluationConfig;

#[cfg(feature = "trace")]
use crate::trace::{ExpressionNode, TraceCollector, TracedResult};
use crate::{CompiledLogic, CompiledNode, Result};
#[cfg(feature = "compat")]
use crate::{Error, StructuredError};

/// The main DataLogic engine for compiling and evaluating JSONLogic expressions.
///
/// The engine provides a two-phase approach to logic evaluation:
/// 1. **Compilation**: Parse JSON logic into optimized `CompiledLogic`
/// 2. **Evaluation**: Execute compiled logic against data
///
/// # Features
///
/// - **Thread-safe**: Compiled logic can be shared across threads with `Arc`
/// - **Extensible**: Add custom operators via `add_operator`
/// - **Structure preservation**: Optionally preserve object structure for templating
/// - **OpCode dispatch**: Built-in operators use fast enum-based dispatch
///
/// # Example
///
/// ```rust
/// use datalogic_rs::DataLogic;
///
/// let engine = DataLogic::new();
/// let result = engine.evaluate_str(
///     r#"{">": [{"var": "age"}, 18]}"#,
///     r#"{"age": 21}"#,
/// ).unwrap();
/// assert_eq!(result, "true");
/// ```
pub struct DataLogic {
    /// Custom `DataOperator` implementations registered with the engine.
    pub(super) custom_arena_operators: HashMap<String, Box<dyn crate::DataOperator>>,
    /// Flag to preserve structure of objects with unknown operators
    #[cfg(feature = "preserve")]
    preserve_structure: bool,
    /// Configuration for evaluation behavior
    config: EvaluationConfig,
}

mod dispatch;

/// Cold fallback for `CompiledNode::Value { arena_lit: None, .. }` — only
/// reached by ad-hoc `synthetic_value` wrappers (test helpers, trace nodes
/// built outside `CompiledLogic::new`). Outlined so the inliner doesn't
/// expand it into the hot `evaluate_node` literal arm.
#[cold]
#[inline(never)]
fn literal_fallback_arena<'a>(
    value: &'a datavalue::OwnedDataValue,
    arena: &'a bumpalo::Bump,
) -> &'a crate::arena::DataValue<'a> {
    use datavalue::OwnedDataValue;
    match value {
        OwnedDataValue::Null => crate::arena::pool::singleton_null(),
        OwnedDataValue::Bool(b) => crate::arena::pool::singleton_bool(*b),
        OwnedDataValue::String(s) if s.is_empty() => crate::arena::pool::singleton_empty_string(),
        OwnedDataValue::Array(a) if a.is_empty() => crate::arena::pool::singleton_empty_array(),
        OwnedDataValue::String(s) => arena.alloc(crate::arena::DataValue::String(s.as_str())),
        _ => arena.alloc(value.to_arena(arena)),
    }
}

impl Default for DataLogic {
    fn default() -> Self {
        Self::new()
    }
}

impl DataLogic {
    /// Start a [`crate::DataLogicBuilder`] for fluent construction.
    ///
    /// Replaces the 4.x `new` / `with_preserve_structure` / `with_config` /
    /// `with_config_and_structure` constructors. The four old methods are
    /// still reachable through `crate::compat::DataLogicLegacyExt`.
    #[inline]
    pub fn builder() -> crate::DataLogicBuilder {
        crate::DataLogicBuilder::new()
    }

    /// Internal seam used by the builder. Not part of the public API.
    #[doc(hidden)]
    #[inline]
    pub(crate) fn from_builder_parts(
        config: EvaluationConfig,
        _preserve_structure: bool,
        operators: HashMap<String, Box<dyn crate::DataOperator>>,
    ) -> Self {
        Self {
            custom_arena_operators: operators,
            #[cfg(feature = "preserve")]
            preserve_structure: _preserve_structure,
            config,
        }
    }

    /// Internal constructor — single source of truth for the four public
    /// `new`/`with_*` variants. `_preserve_structure` is parameterised here
    /// so non-`preserve` builds can ignore it without four near-duplicate
    /// `Self { ... }` blocks.
    #[inline]
    fn new_inner(config: EvaluationConfig, _preserve_structure: bool) -> Self {
        Self {
            custom_arena_operators: HashMap::new(),
            #[cfg(feature = "preserve")]
            preserve_structure: _preserve_structure,
            config,
        }
    }

    /// Creates a new DataLogic engine with all built-in operators.
    ///
    /// The engine includes 50+ built-in operators optimized with OpCode dispatch.
    /// Structure preservation is disabled by default.
    ///
    /// # Example
    ///
    /// ```rust
    /// use datalogic_rs::DataLogic;
    ///
    /// let engine = DataLogic::new();
    /// ```
    pub fn new() -> Self {
        Self::new_inner(EvaluationConfig::default(), false)
    }

    /// Creates a new DataLogic engine with structure preservation enabled.
    ///
    /// When enabled, objects with unknown operators are preserved as structured
    /// templates, allowing for dynamic object generation. Custom operators
    /// registered via `add_operator` are recognized and evaluated properly,
    /// even within structured objects.
    ///
    /// # Example
    ///
    /// ```rust
    /// use datalogic_rs::DataLogic;
    /// use serde_json::json;
    ///
    /// let engine = DataLogic::with_preserve_structure();
    /// let logic = json!({
    ///     "name": {"var": "user.name"},
    ///     "score": {"+": [{"var": "base"}, {"var": "bonus"}]}
    /// });
    /// // Returns: {"name": "Alice", "score": 95}
    /// ```
    ///
    /// # Custom Operators with Preserve Structure
    ///
    /// Custom operators work seamlessly in preserve_structure mode:
    ///
    /// ```rust
    /// use bumpalo::Bump;
    /// use datalogic_rs::{DataContextStack, DataOperator, DataValue, DataLogic, Result};
    ///
    /// struct UpperOperator;
    /// impl DataOperator for UpperOperator {
    ///     fn evaluate<'a>(
    ///         &self,
    ///         args: &[&'a DataValue<'a>],
    ///         _actx: &mut DataContextStack<'a>,
    ///         arena: &'a Bump,
    ///     ) -> Result<&'a DataValue<'a>> {
    ///         let s = args[0].as_str().unwrap_or("").to_uppercase();
    ///         Ok(arena.alloc(DataValue::String(arena.alloc_str(&s))))
    ///     }
    /// }
    ///
    /// let mut engine = DataLogic::with_preserve_structure();
    /// engine.add_operator("upper".to_string(), Box::new(UpperOperator));
    ///
    /// let result = engine.evaluate_str(
    ///     r#"{"message": {"upper": {"var": "text"}}, "count": {"var": "num"}}"#,
    ///     r#"{"text": "hello", "num": 5}"#,
    /// ).unwrap();
    /// // result == r#"{"message":"HELLO","count":5}"#
    /// ```
    #[cfg(feature = "preserve")]
    pub fn with_preserve_structure() -> Self {
        Self::new_inner(EvaluationConfig::default(), true)
    }

    /// Creates a new DataLogic engine with a custom configuration.
    ///
    /// # Arguments
    ///
    /// * `config` - The evaluation configuration
    ///
    /// # Example
    ///
    /// ```rust
    /// use datalogic_rs::{DataLogic, EvaluationConfig, NanHandling};
    ///
    /// let config = EvaluationConfig::default()
    ///     .with_nan_handling(NanHandling::IgnoreValue);
    /// let engine = DataLogic::with_config(config);
    /// ```
    pub fn with_config(config: EvaluationConfig) -> Self {
        Self::new_inner(config, false)
    }

    /// Creates a new DataLogic engine with both configuration and structure preservation.
    ///
    /// # Arguments
    ///
    /// * `config` - The evaluation configuration
    /// * `preserve_structure` - Whether to preserve object structure
    ///
    /// # Example
    ///
    /// ```rust
    /// use datalogic_rs::{DataLogic, EvaluationConfig, NanHandling};
    ///
    /// let config = EvaluationConfig::default()
    ///     .with_nan_handling(NanHandling::IgnoreValue);
    /// let engine = DataLogic::with_config_and_structure(config, true);
    /// ```
    #[cfg(feature = "preserve")]
    pub fn with_config_and_structure(config: EvaluationConfig, preserve_structure: bool) -> Self {
        Self::new_inner(config, preserve_structure)
    }

    /// Gets a reference to the current evaluation configuration.
    pub fn config(&self) -> &EvaluationConfig {
        &self.config
    }

    /// Returns whether structure preservation is enabled.
    pub fn preserve_structure(&self) -> bool {
        #[cfg(feature = "preserve")]
        {
            self.preserve_structure
        }
        #[cfg(not(feature = "preserve"))]
        {
            false
        }
    }

    /// Registers a custom [`crate::DataOperator`] with the engine.
    ///
    /// Implementations take pre-evaluated args as `&'a DataValue<'a>` and
    /// return an arena-allocated result.
    ///
    /// # Example
    ///
    /// ```rust
    /// use datalogic_rs::{DataContextStack, DataOperator, DataValue, DataLogic, Result};
    /// use bumpalo::Bump;
    ///
    /// struct Plus42;
    /// impl DataOperator for Plus42 {
    ///     fn evaluate<'a>(
    ///         &self,
    ///         args: &[&'a DataValue<'a>],
    ///         _actx: &mut DataContextStack<'a>,
    ///         arena: &'a Bump,
    ///     ) -> Result<&'a DataValue<'a>> {
    ///         let n = args.first().and_then(|v| v.as_f64()).unwrap_or(0.0);
    ///         Ok(arena.alloc(DataValue::from_f64(n + 42.0)))
    ///     }
    /// }
    ///
    /// let mut engine = DataLogic::new();
    /// engine.add_operator("plus42".into(), Box::new(Plus42));
    /// let result = engine.evaluate_str(r#"{"plus42": 8}"#, "null").unwrap();
    /// assert_eq!(result, "50");
    /// ```
    pub fn add_operator(&mut self, name: String, operator: Box<dyn crate::DataOperator>) {
        self.custom_arena_operators.insert(name, operator);
    }

    /// Checks if a custom operator with the given name is registered.
    ///
    /// # Arguments
    ///
    /// * `name` - The operator name to check
    ///
    /// # Returns
    ///
    /// `true` if the operator exists, `false` otherwise.
    pub fn has_custom_operator(&self, name: &str) -> bool {
        self.custom_arena_operators.contains_key(name)
    }

    // ============================================================
    // V5 PUBLIC API — power users compile once + evaluate many; normal
    // users call `evaluate_str` directly.
    // ============================================================

    /// Compile a JSON logic string into reusable [`CompiledLogic`].
    ///
    /// The canonical v5 entry point for compilation. Returns an
    /// `Arc<CompiledLogic>` that can be reused across many evaluations and
    /// shared across threads.
    ///
    /// # Example
    ///
    /// ```rust
    /// use datalogic_rs::DataLogic;
    ///
    /// let engine = DataLogic::new();
    /// let compiled = engine.compile(r#"{"==": [{"var": "x"}, 1]}"#).unwrap();
    /// ```
    pub fn compile(&self, logic: &str) -> Result<Arc<CompiledLogic>> {
        let owned = datavalue::OwnedDataValue::from_json(logic)?;
        self.compile_value(&owned)
    }

    /// Internal compile helper shared by [`Self::compile`] and the compat
    /// `compile_serde_value` shim. Not part of the public API.
    #[doc(hidden)]
    pub(crate) fn compile_value(
        &self,
        logic: &datavalue::OwnedDataValue,
    ) -> Result<Arc<CompiledLogic>> {
        let compiled = CompiledLogic::compile_with_static_eval(logic, self)?;
        Ok(Arc::new(compiled))
    }

    /// Evaluate compiled logic against arena-resident data.
    ///
    /// The hot path for repeated evaluation. The caller owns the
    /// [`bumpalo::Bump`] lifecycle and may `reset()` it between calls; the
    /// returned `&DataValue<'a>` borrows from the arena, so it must be
    /// dropped before the next reset (enforced by the borrow checker).
    ///
    /// Pre-parse JSON input via
    /// [`datavalue::DataValue::from_str`](datavalue::DataValue::from_str)
    /// to obtain the `&DataValue<'a>` data argument.
    ///
    /// # Example
    ///
    /// ```rust
    /// use bumpalo::Bump;
    /// use datalogic_rs::{DataLogic, DataValue};
    ///
    /// let engine = DataLogic::new();
    /// let compiled = engine.compile(r#"{"+": [{"var": "x"}, 2]}"#).unwrap();
    ///
    /// let arena = Bump::new();
    /// let data = DataValue::from_str(r#"{"x": 40}"#, &arena).unwrap();
    /// let result = engine.evaluate(&compiled, arena.alloc(data), &arena).unwrap();
    /// assert_eq!(result.as_i64(), Some(42));
    /// ```
    #[inline(always)]
    pub fn evaluate<'a>(
        &self,
        compiled: &'a CompiledLogic,
        data: &'a crate::arena::DataValue<'a>,
        arena: &'a bumpalo::Bump,
    ) -> Result<&'a crate::arena::DataValue<'a>> {
        let mut actx = crate::arena::DataContextStack::new(data);
        self.evaluate_node(&compiled.root, &mut actx, arena)
    }

    /// One-shot evaluation with JSON-string boundary on both sides.
    ///
    /// Parses `logic` + `data`, evaluates, and returns the result as a JSON
    /// `String`. Allocates a fresh [`bumpalo::Bump`] internally — for
    /// repeated calls against the same rule, prefer [`Self::compile`] +
    /// [`Self::evaluate`] with a reused arena.
    ///
    /// # Example
    ///
    /// ```rust
    /// use datalogic_rs::DataLogic;
    ///
    /// let engine = DataLogic::new();
    /// let result = engine.evaluate_str(
    ///     r#"{"==": [{"var": "x"}, 5]}"#,
    ///     r#"{"x": 5}"#,
    /// ).unwrap();
    /// assert_eq!(result, "true");
    /// ```
    pub fn evaluate_str(&self, logic: &str, data: &str) -> Result<String> {
        let compiled = self.compile(logic)?;
        let arena = bumpalo::Bump::new();
        let data_dv = datavalue::DataValue::from_str(data, &arena)?;
        let result = self.evaluate(&compiled, arena.alloc(data_dv), &arena)?;
        Ok(crate::arena::data_to_json_string(result))
    }

    /// One-shot evaluation with `serde_json::Value` boundary on both sides.
    ///
    /// Mirror of [`Self::evaluate_str`] for callers already on `serde_json`.
    /// Funnels through [`Self::evaluate`] internally.
    #[cfg(feature = "compat")]
    pub fn evaluate_value(&self, logic: &Value, data: &Value) -> Result<Value> {
        let logic_owned = crate::value::owned_from_serde(logic);
        let compiled = self.compile_value(&logic_owned)?;
        let arena = bumpalo::Bump::new();
        let data_av = crate::arena::value_to_arena(data, &arena);
        let result = self.evaluate(&compiled, arena.alloc(data_av), &arena)?;
        Ok(crate::arena::arena_to_value(result))
    }

    // ============================================================
    // DEPRECATED v4 COMPAT — all funnel through v5 `evaluate` /
    // `compile_value`. Removed in 5.1.
    // ============================================================

    /// Deprecated: use [`Self::compile`] (`&str`) or
    /// [`Self::evaluate_value`] (`&Value, &Value`) for one-shot.
    #[cfg(feature = "compat")]
    #[deprecated(
        since = "5.0.0",
        note = "use `compile(&str)` or `evaluate_value(&Value, &Value)` for one-shot"
    )]
    pub fn compile_serde_value(&self, logic: &Value) -> Result<Arc<CompiledLogic>> {
        let owned = crate::value::owned_from_serde(logic);
        self.compile_value(&owned)
    }

    /// Deprecated: use [`Self::evaluate`] (`&CompiledLogic, &DataValue, &Bump`).
    #[cfg(feature = "compat")]
    #[deprecated(
        since = "5.0.0",
        note = "use `evaluate(&CompiledLogic, &DataValue, &Bump)` or `evaluate_value(&Value, &Value)`"
    )]
    pub fn evaluate_arc_value(&self, compiled: &CompiledLogic, data: Arc<Value>) -> Result<Value> {
        self.eval_to_value(compiled, &data)
    }

    /// Deprecated: use [`Self::evaluate`] or [`Self::evaluate_value`].
    #[cfg(feature = "compat")]
    #[deprecated(
        since = "5.0.0",
        note = "use `evaluate(&CompiledLogic, &DataValue, &Bump)` or `evaluate_value(&Value, &Value)`"
    )]
    pub fn evaluate_ref(&self, compiled: &CompiledLogic, data: &Value) -> Result<Value> {
        self.eval_to_value(compiled, data)
    }

    /// Deprecated: use [`Self::evaluate`] or [`Self::evaluate_value`].
    #[cfg(feature = "compat")]
    #[deprecated(
        since = "5.0.0",
        note = "use `evaluate(&CompiledLogic, &DataValue, &Bump)` or `evaluate_value(&Value, &Value)`"
    )]
    pub fn evaluate_owned(&self, compiled: &CompiledLogic, data: Value) -> Result<Value> {
        self.eval_to_value(compiled, &data)
    }

    /// Deprecated: use [`Self::evaluate_str`] or [`Self::evaluate_value`].
    #[cfg(feature = "compat")]
    #[deprecated(
        since = "5.0.0",
        note = "use `evaluate_str(&str, &str)` or `evaluate_value(&Value, &Value)`"
    )]
    pub fn evaluate_json(&self, logic: &str, data: &str) -> Result<Value> {
        let logic_value: Value = serde_json::from_str(logic)?;
        let data_value: Value = serde_json::from_str(data)?;
        self.evaluate_value(&logic_value, &data_value)
    }

    /// Internal `&Value -> Value` adapter shared by the compat eval shims.
    /// Funnels through public [`Self::evaluate`] so the dispatch path is
    /// identical to the v5 entry.
    #[cfg(feature = "compat")]
    #[doc(hidden)]
    pub(crate) fn eval_to_value(&self, compiled: &CompiledLogic, data: &Value) -> Result<Value> {
        let arena = bumpalo::Bump::new();
        let data_av = crate::arena::value_to_arena(data, &arena);
        let result = self.evaluate(compiled, arena.alloc(data_av), &arena)?;
        Ok(crate::arena::arena_to_value(result))
    }

    /// Deprecated: structured-error variant of the compat evaluate.
    /// Calls `evaluate_node` directly because the structured error path
    /// needs the [`crate::arena::DataContextStack`] after evaluation to
    /// extract the error breadcrumb. A v5 sibling will land alongside a
    /// dedicated structured-error API.
    #[cfg(feature = "compat")]
    #[deprecated(
        since = "5.0.0",
        note = "use `evaluate_value` (returns Result<Value, Error>); a v5 structured-error API will replace this"
    )]
    pub fn evaluate_structured(
        &self,
        compiled: &CompiledLogic,
        data: Arc<Value>,
    ) -> std::result::Result<Value, StructuredError> {
        let arena = bumpalo::Bump::new();
        let data_av = crate::arena::value_to_arena(&data, &arena);
        let mut actx = crate::arena::DataContextStack::new(arena.alloc(data_av));
        match self.evaluate_node(&compiled.root, &mut actx, &arena) {
            Ok(av) => Ok(crate::arena::arena_to_value(av)),
            Err(e) => {
                let path = actx.take_error_path();
                let mut se = StructuredError::from(e).with_path(path);
                if let Some(name) = compiled.root.operator_name() {
                    se = se.with_operator(name);
                }
                Err(se)
            }
        }
    }

    /// Deprecated: parse + compile + structured evaluate.
    #[cfg(feature = "compat")]
    #[deprecated(
        since = "5.0.0",
        note = "use `evaluate_str` / `evaluate_value`; a v5 structured-error API will replace this"
    )]
    pub fn evaluate_json_structured(
        &self,
        logic: &str,
        data: &str,
    ) -> std::result::Result<Value, StructuredError> {
        let logic_value: Value = serde_json::from_str(logic).map_err(Error::from)?;
        let data_value: Value = serde_json::from_str(data).map_err(Error::from)?;
        #[allow(deprecated)]
        let compiled = self.compile_serde_value(&logic_value)?;
        #[allow(deprecated)]
        self.evaluate_structured(&compiled, Arc::new(data_value))
    }

    /// Arena-mode dispatch hub. Returns `&'a DataValue<'a>` for every
    /// `CompiledNode` shape — exhaustive match, no value-mode fallback.
    ///
    /// On error, accumulates the failing node's id onto the context stack's
    /// breadcrumb so [`StructuredError`] consumers can surface the failing
    /// path. When a tracer is attached to `actx`, records a step per
    /// non-literal node (entry context + result/error).
    #[inline(always)]
    pub(crate) fn evaluate_node<'a>(
        &self,
        node: &'a CompiledNode,
        actx: &mut crate::arena::DataContextStack<'a>,
        arena: &'a bumpalo::Bump,
    ) -> Result<&'a crate::arena::DataValue<'a>> {
        // Literal fast path — no breadcrumb push, no trace step.
        if let CompiledNode::Value {
            value, arena_lit, ..
        } = node
        {
            // Compiled-tree literals always have `arena_lit` populated by
            // `populate_arena_lits` (run during `CompiledLogic::new`), so
            // this branch covers every literal in any finalized rule.
            // DataValue is covariant in its lifetime, so
            // `&'a DataValue<'static>` satisfies `&'a DataValue<'a>`
            // without unsafe.
            if let Some(av) = arena_lit {
                return Ok(av);
            }
            // Fallback for nodes built outside the compile pipeline (test
            // helpers in `trace.rs`, ad-hoc `synthetic_value` wrappers that
            // never went through `CompiledLogic::new`). Outlined + cold so
            // the literal fast path stays a single load+branch in the
            // dispatched dominant case.
            return Ok(literal_fallback_arena(value, arena));
        }

        // Snapshot context for trace BEFORE recursing — children will
        // mutate iteration frames. Cheap when no tracer is attached.
        #[cfg(feature = "trace")]
        let ctx_snapshot: Option<Value> = actx.has_tracer().then(|| actx.current_data_as_value());

        let result = dispatch::evaluate_node_inner(self, node, actx, arena);

        // Accumulate the failing node's id on every Err. We always pay
        // the (single) Vec::push since errors are rare and structured-error
        // consumers need the breadcrumb.
        if result.is_err() {
            actx.push_error_step(node.id());
        }

        #[cfg(feature = "trace")]
        if let Some(ctx_data) = ctx_snapshot {
            actx.record_node_result(node.id(), ctx_data, &result);
        }

        result
    }

    /// Evaluate an iteration body (map/filter/reduce/all/some/none) with the
    /// trace collector's iteration index/total markers set around it. The
    /// markers are no-ops when no tracer is attached, so plain-mode callers
    /// pay only one branch per iteration.
    #[inline]
    pub(crate) fn eval_iter_body<'a>(
        &self,
        body: &'a CompiledNode,
        actx: &mut crate::arena::DataContextStack<'a>,
        arena: &'a bumpalo::Bump,
        _index: u32,
        _total: u32,
    ) -> Result<&'a crate::arena::DataValue<'a>> {
        #[cfg(feature = "trace")]
        actx.trace_push_iteration(_index, _total);
        let res = self.evaluate_node(body, actx, arena);
        #[cfg(feature = "trace")]
        actx.trace_pop_iteration();
        res
    }

    /// Deprecated: traced evaluation with serde_json boundary. A v5 trace
    /// surface will land in 5.1 alongside the structured-error v5 API.
    #[cfg(feature = "trace")]
    #[deprecated(
        since = "5.0.0",
        note = "the v5 trace surface lands in 5.1; this method will be removed at that time"
    )]
    pub fn evaluate_json_with_trace(&self, logic: &str, data: &str) -> Result<TracedResult> {
        let logic_value: Value = serde_json::from_str(logic)?;
        let data_value: Value = serde_json::from_str(data)?;
        let data_arc = Arc::new(data_value);
        let compiled = self.compile_for_trace(&logic_value)?;
        Ok(self.run_trace(&compiled, data_arc))
    }

    /// Compile a value tree for traced evaluation — `compile_for_trace` skips
    /// static evaluation so every operator stays in the tree as a step source.
    #[cfg(feature = "trace")]
    fn compile_for_trace(&self, logic_value: &Value) -> Result<Arc<CompiledLogic>> {
        let owned = crate::value::owned_from_serde(logic_value);
        Ok(Arc::new(CompiledLogic::compile_for_trace(
            &owned,
            self.preserve_structure(),
        )?))
    }

    /// Run a traced evaluation and assemble the [`TracedResult`]. Shared
    /// between [`evaluate_json_with_trace`] and
    /// [`evaluate_json_with_trace_structured`].
    #[cfg(feature = "trace")]
    fn run_trace(&self, compiled: &CompiledLogic, data_arc: Arc<Value>) -> TracedResult {
        let expression_tree = ExpressionNode::build_from_compiled(&compiled.root);
        let mut collector = TraceCollector::new();
        let (result, error_path) = self.evaluate_with_trace(compiled, data_arc, &mut collector);
        let steps = collector.into_steps();
        match result {
            Ok(value) => TracedResult {
                result: value,
                expression_tree,
                steps,
                error: None,
                error_structured: None,
            },
            Err(e) => {
                let message = e.to_string();
                let mut structured = StructuredError::from(e).with_path(error_path);
                if let Some(name) = compiled.root.operator_name() {
                    structured = structured.with_operator(name);
                }
                TracedResult {
                    result: Value::Null,
                    expression_tree,
                    steps,
                    error: Some(message),
                    error_structured: Some(structured),
                }
            }
        }
    }

    /// Deprecated: structured-error trace variant. A v5 trace surface will
    /// land in 5.1.
    #[cfg(feature = "trace")]
    #[deprecated(
        since = "5.0.0",
        note = "the v5 trace surface lands in 5.1; this method will be removed at that time"
    )]
    pub fn evaluate_json_with_trace_structured(
        &self,
        logic: &str,
        data: &str,
    ) -> std::result::Result<TracedResult, StructuredError> {
        let logic_value: Value = serde_json::from_str(logic).map_err(Error::from)?;
        let data_value: Value = serde_json::from_str(data).map_err(Error::from)?;
        let data_arc = Arc::new(data_value);
        let compiled = self.compile_for_trace(&logic_value)?;
        Ok(self.run_trace(&compiled, data_arc))
    }

    /// Arena-mode traced evaluation. Allocates an arena, attaches the
    /// caller's [`TraceCollector`] to the arena context, and dispatches
    /// through [`evaluate_node`]. Returns `(result, error_path)` where
    /// `error_path` is the structured-error breadcrumb of node ids leading
    /// to the failure (empty on success). Calls `evaluate_node` directly
    /// (not the public [`Self::evaluate`]) because the trace path needs the
    /// [`crate::arena::DataContextStack`] both before (to attach the tracer)
    /// and after (to extract the breadcrumb) the evaluation.
    #[cfg(feature = "trace")]
    fn evaluate_with_trace(
        &self,
        compiled: &CompiledLogic,
        data: Arc<Value>,
        collector: &mut TraceCollector,
    ) -> (Result<Value>, Vec<u32>) {
        let arena = bumpalo::Bump::new();
        let data_av = crate::arena::value_to_arena(&data, &arena);
        let mut actx = crate::arena::DataContextStack::new(arena.alloc(data_av));
        actx.set_tracer(collector);
        let result = self.evaluate_node(&compiled.root, &mut actx, &arena);
        match result {
            Ok(av) => {
                let owned = crate::arena::arena_to_value(av);
                let path = actx.take_error_path();
                (Ok(owned), path)
            }
            Err(e) => {
                let path = actx.take_error_path();
                (Err(e), path)
            }
        }
    }
}
