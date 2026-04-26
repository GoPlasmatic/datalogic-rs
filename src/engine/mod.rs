use serde_json::Value;
use std::collections::HashMap;
use std::sync::Arc;

use crate::config::EvaluationConfig;

#[cfg(feature = "trace")]
use crate::trace::{ExpressionNode, TraceCollector, TracedResult};
use crate::{CompiledLogic, CompiledNode, Error, Result, StructuredError};

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
/// use serde_json::json;
///
/// let engine = DataLogic::new();
/// let logic = json!({">": [{"var": "age"}, 18]});
/// let compiled = engine.compile(&logic).unwrap();
///
/// let data = json!({"age": 21});
/// let result = engine.evaluate_owned(&compiled, data).unwrap();
/// assert_eq!(result, json!(true));
/// ```
pub struct DataLogic {
    /// Custom `ArenaOperator` implementations registered with the engine.
    pub(super) custom_arena_operators: HashMap<String, Box<dyn crate::ArenaOperator>>,
    /// Flag to preserve structure of objects with unknown operators
    #[cfg(feature = "preserve")]
    preserve_structure: bool,
    /// Configuration for evaluation behavior
    config: EvaluationConfig,
}

mod dispatch;

impl Default for DataLogic {
    fn default() -> Self {
        Self::new()
    }
}

impl DataLogic {
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
        Self {
            custom_arena_operators: HashMap::new(),
            #[cfg(feature = "preserve")]
            preserve_structure: false,
            config: EvaluationConfig::default(),
        }
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
    /// use datalogic_rs::{ArenaContextStack, ArenaOperator, ArenaValue, DataLogic, Result};
    /// use serde_json::{json, Value};
    /// use std::sync::Arc;
    ///
    /// struct UpperOperator;
    /// impl ArenaOperator for UpperOperator {
    ///     fn evaluate_arena<'a>(
    ///         &self,
    ///         args: &[&'a ArenaValue<'a>],
    ///         _actx: &mut ArenaContextStack<'a>,
    ///         arena: &'a Bump,
    ///     ) -> Result<&'a ArenaValue<'a>> {
    ///         let s = args[0].as_str().unwrap_or("").to_uppercase();
    ///         Ok(arena.alloc(ArenaValue::String(arena.alloc_str(&s))))
    ///     }
    /// }
    ///
    /// let mut engine = DataLogic::with_preserve_structure();
    /// engine.add_arena_operator("upper".to_string(), Box::new(UpperOperator));
    ///
    /// let logic = json!({
    ///     "message": {"upper": {"var": "text"}},
    ///     "count": {"var": "num"}
    /// });
    /// let compiled = engine.compile(&logic).unwrap();
    /// let result = engine.evaluate(&compiled, Arc::new(json!({"text": "hello", "num": 5}))).unwrap();
    /// // Returns: {"message": "HELLO", "count": 5}
    /// ```
    #[cfg(feature = "preserve")]
    pub fn with_preserve_structure() -> Self {
        Self {
            custom_arena_operators: HashMap::new(),
            preserve_structure: true,
            config: EvaluationConfig::default(),
        }
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
        Self {
            custom_arena_operators: HashMap::new(),
            #[cfg(feature = "preserve")]
            preserve_structure: false,
            config,
        }
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
        Self {
            custom_arena_operators: HashMap::new(),
            preserve_structure,
            config,
        }
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

    /// Registers a custom [`crate::ArenaOperator`] with the engine.
    ///
    /// Implementations take pre-evaluated args as `&'a ArenaValue<'a>` and
    /// return an arena-allocated result.
    ///
    /// # Example
    ///
    /// ```rust
    /// use datalogic_rs::{ArenaContextStack, ArenaOperator, ArenaValue, DataLogic, Result};
    /// use bumpalo::Bump;
    /// use serde_json::json;
    ///
    /// struct Plus42;
    /// impl ArenaOperator for Plus42 {
    ///     fn evaluate_arena<'a>(
    ///         &self,
    ///         args: &[&'a ArenaValue<'a>],
    ///         _actx: &mut ArenaContextStack<'a>,
    ///         arena: &'a Bump,
    ///     ) -> Result<&'a ArenaValue<'a>> {
    ///         let n = args.first().and_then(|v| v.as_f64()).unwrap_or(0.0);
    ///         Ok(arena.alloc(ArenaValue::from_f64(n + 42.0)))
    ///     }
    /// }
    ///
    /// let mut engine = DataLogic::new();
    /// engine.add_arena_operator("plus42".into(), Box::new(Plus42));
    /// let compiled = engine.compile(&json!({"plus42": 8})).unwrap();
    /// assert_eq!(engine.evaluate_ref(&compiled, &json!({})).unwrap(), json!(50));
    /// ```
    pub fn add_arena_operator(&mut self, name: String, operator: Box<dyn crate::ArenaOperator>) {
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

    /// Compiles a JSON logic expression into an optimized form.
    ///
    /// Compilation performs:
    /// - Static evaluation of constant expressions
    /// - OpCode assignment for built-in operators
    /// - Structure analysis for templating
    ///
    /// The returned `Arc<CompiledLogic>` can be safely shared across threads.
    ///
    /// # Arguments
    ///
    /// * `logic` - The JSON logic expression to compile
    ///
    /// # Returns
    ///
    /// An `Arc`-wrapped compiled logic structure, or an error if compilation fails.
    ///
    /// # Example
    ///
    /// ```rust
    /// use datalogic_rs::DataLogic;
    /// use serde_json::json;
    /// use std::sync::Arc;
    ///
    /// let engine = DataLogic::new();
    /// let logic = json!({"==": [1, 1]});
    /// let compiled: Arc<_> = engine.compile(&logic).unwrap();
    /// ```
    pub fn compile(&self, logic: &Value) -> Result<Arc<CompiledLogic>> {
        let compiled = CompiledLogic::compile_with_static_eval(logic, self)?;
        Ok(Arc::new(compiled))
    }

    /// Evaluates compiled logic with Arc-wrapped data.
    ///
    /// Use this method when you already have data in an `Arc` to avoid re-wrapping.
    /// For owned data, use `evaluate_owned` instead. For borrowed data, use
    /// `evaluate_ref` to skip the Arc altogether.
    ///
    /// # Arguments
    ///
    /// * `compiled` - The compiled logic to evaluate
    /// * `data` - The data context wrapped in an `Arc`
    ///
    /// # Returns
    ///
    /// The evaluation result, or an error if evaluation fails.
    pub fn evaluate(&self, compiled: &CompiledLogic, data: Arc<Value>) -> Result<Value> {
        self.eval_to_value(compiled, &data)
    }

    /// Evaluates compiled logic against borrowed data.
    ///
    /// This is the canonical fast path — no `Arc` is required. Use this when
    /// you have a `&Value` (e.g., from a parsed input) and don't need to share
    /// the data across threads. For Arc-wrapped data use [`evaluate`]; for
    /// owned data use [`evaluate_owned`].
    ///
    /// # Arguments
    ///
    /// * `compiled` - The compiled logic to evaluate
    /// * `data` - The data context (borrowed)
    ///
    /// # Returns
    ///
    /// The evaluation result, or an error if evaluation fails.
    pub fn evaluate_ref(&self, compiled: &CompiledLogic, data: &Value) -> Result<Value> {
        self.eval_to_value(compiled, data)
    }

    /// Arena-mode evaluation entry. Acquires a thread-local `Bump` (from the
    /// pool, or freshly sized from the rule's compile-time hint), dispatches
    /// through `evaluate_arena_node`, and converts the result back to owned
    /// `Value` at the boundary. The arena is reset and returned to the pool
    /// when `guard` drops at end of function.
    #[inline]
    fn eval_to_value(&self, compiled: &CompiledLogic, data: &Value) -> Result<Value> {
        use crate::arena::{ArenaGuard, arena_to_value};
        // Size hint for first-time pool fills: static_bytes × 2, min 4 KiB.
        let cap = compiled.arena_static_bytes.saturating_mul(2).max(4096);
        let guard = ArenaGuard::acquire(cap);
        let arena = guard.arena();
        let mut actx = crate::arena::ArenaContextStack::from_value(data, arena);
        let result = self.evaluate_arena_node(&compiled.root, &mut actx, arena)?;
        let owned = arena_to_value(result);
        drop(guard);
        Ok(owned)
    }

    /// Evaluates compiled logic with owned data.
    ///
    /// This is a convenience method that wraps the data in an `Arc` before evaluation.
    /// If you already have Arc-wrapped data, use `evaluate` instead.
    ///
    /// # Arguments
    ///
    /// * `compiled` - The compiled logic to evaluate
    /// * `data` - The owned data context
    ///
    /// # Returns
    ///
    /// The evaluation result, or an error if evaluation fails.
    ///
    /// # Example
    ///
    /// ```rust
    /// use datalogic_rs::DataLogic;
    /// use serde_json::json;
    ///
    /// let engine = DataLogic::new();
    /// let logic = json!({"var": "name"});
    /// let compiled = engine.compile(&logic).unwrap();
    ///
    /// let data = json!({"name": "Alice"});
    /// let result = engine.evaluate_owned(&compiled, data).unwrap();
    /// assert_eq!(result, json!("Alice"));
    /// ```
    pub fn evaluate_owned(&self, compiled: &CompiledLogic, data: Value) -> Result<Value> {
        self.evaluate(compiled, Arc::new(data))
    }

    // (evaluate_ref is the canonical zero-Arc path; see definition above.)

    /// Pure arena evaluation against a caller-provided `Bump`. Returns the
    /// arena-allocated result without converting to `serde_json::Value`, and
    /// without touching the thread-local `ArenaGuard` slot — the caller owns
    /// the arena's lifecycle and decides when to `reset()` it. Returned
    /// `&'a ArenaValue<'a>` borrows from `arena`, so it must drop before the
    /// next `arena.reset()` (the borrow checker enforces this).
    ///
    /// `data` is `&'a ArenaValue<'a>` so callers operate consistently in
    /// arena terms. Deep-convert an existing `&Value` via
    /// `arena.alloc(value_to_arena(value, arena))` — primitives stay inline,
    /// composites are allocated in the arena.
    ///
    /// Used by `examples/benchmark.rs` to measure dispatch in isolation by
    /// creating one arena up-front and resetting only between rules,
    /// excluding the per-call ArenaGuard pop/push from the measurement.
    /// Not part of the stable API.
    #[doc(hidden)]
    #[inline]
    pub fn evaluate_in_arena<'a>(
        &self,
        compiled: &'a CompiledLogic,
        data: &'a crate::arena::ArenaValue<'a>,
        arena: &'a bumpalo::Bump,
    ) -> Result<&'a crate::arena::ArenaValue<'a>> {
        let mut actx = crate::arena::ArenaContextStack::new(data);
        self.evaluate_arena_node(&compiled.root, &mut actx, arena)
    }

    /// Pure arena evaluation for benchmarking — runs `evaluate_arena_node`
    /// against an internally-acquired `ArenaGuard`. Kept as the equivalent
    /// of the public `evaluate*` API minus the `arena_to_value` boundary,
    /// so callers can compare dispatch-only cost with vs. without the
    /// thread-local arena slot. Not part of the stable API.
    #[doc(hidden)]
    pub fn evaluate_arena_bench(&self, compiled: &CompiledLogic, data: &Value) -> Result<()> {
        use crate::arena::ArenaGuard;
        let cap = compiled.arena_static_bytes.saturating_mul(2).max(4096);
        let guard = ArenaGuard::acquire(cap);
        let arena = guard.arena();
        let mut actx = crate::arena::ArenaContextStack::from_value(data, arena);
        let result = self.evaluate_arena_node(&compiled.root, &mut actx, arena)?;
        std::hint::black_box(result);
        drop(guard);
        Ok(())
    }

    /// Convenience method for evaluating JSON strings directly.
    ///
    /// This method combines compilation and evaluation in a single call.
    /// For repeated evaluations, compile once and reuse the compiled logic.
    ///
    /// # Arguments
    ///
    /// * `logic` - JSON logic as a string
    /// * `data` - Data context as a JSON string
    ///
    /// # Returns
    ///
    /// The evaluation result, or an error if parsing or evaluation fails.
    ///
    /// # Example
    ///
    /// ```rust
    /// use datalogic_rs::DataLogic;
    ///
    /// let engine = DataLogic::new();
    /// let result = engine.evaluate_json(
    ///     r#"{"==": [{"var": "x"}, 5]}"#,
    ///     r#"{"x": 5}"#
    /// ).unwrap();
    /// assert_eq!(result, serde_json::json!(true));
    /// ```
    pub fn evaluate_json(&self, logic: &str, data: &str) -> Result<Value> {
        let logic_value: Value = serde_json::from_str(logic)?;
        let data_value: Value = serde_json::from_str(data)?;
        let data_arc = Arc::new(data_value);

        let compiled = self.compile(&logic_value)?;
        self.evaluate(&compiled, data_arc)
    }

    /// Evaluates a compiled rule, returning a `StructuredError` on failure.
    ///
    /// Identical to [`evaluate`](Self::evaluate) on success. On error, the
    /// `Error` is wrapped with the name of the outermost operator in the
    /// compiled logic, so non-Rust consumers can surface typed error
    /// information without parsing `Display` strings.
    pub fn evaluate_structured(
        &self,
        compiled: &CompiledLogic,
        data: Arc<Value>,
    ) -> std::result::Result<Value, StructuredError> {
        use crate::arena::{ArenaGuard, arena_to_value};
        let cap = compiled.arena_static_bytes.saturating_mul(2).max(4096);
        let guard = ArenaGuard::acquire(cap);
        let arena = guard.arena();
        let mut actx = crate::arena::ArenaContextStack::from_value(&data, arena);
        match self.evaluate_arena_node(&compiled.root, &mut actx, arena) {
            Ok(av) => {
                let owned = arena_to_value(av);
                drop(guard);
                Ok(owned)
            }
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

    /// Convenience method: parse, compile, and evaluate with a structured
    /// error on failure. Sibling of [`evaluate_json`](Self::evaluate_json).
    pub fn evaluate_json_structured(
        &self,
        logic: &str,
        data: &str,
    ) -> std::result::Result<Value, StructuredError> {
        let logic_value: Value = serde_json::from_str(logic).map_err(Error::from)?;
        let data_value: Value = serde_json::from_str(data).map_err(Error::from)?;
        let data_arc = Arc::new(data_value);

        let compiled = self.compile(&logic_value)?;
        self.evaluate_structured(&compiled, data_arc)
    }

    /// Arena-mode dispatch hub. Returns `&'a ArenaValue<'a>` for every
    /// `CompiledNode` shape — exhaustive match, no value-mode fallback.
    ///
    /// On error, accumulates the failing node's id onto the context stack's
    /// breadcrumb so [`StructuredError`] consumers can surface the failing
    /// path. When a tracer is attached to `actx`, records a step per
    /// non-literal node (entry context + result/error).
    #[inline]
    pub(crate) fn evaluate_arena_node<'a>(
        &self,
        node: &'a CompiledNode,
        actx: &mut crate::arena::ArenaContextStack<'a>,
        arena: &'a bumpalo::Bump,
    ) -> Result<&'a crate::arena::ArenaValue<'a>> {
        // Literal fast path — no breadcrumb push, no trace step.
        if let CompiledNode::Value {
            value, arena_lit, ..
        } = node
        {
            // Pre-built primitive (Number) — borrow into the CompiledNode.
            // ArenaValue is covariant in its lifetime, so &'a ArenaValue<'static>
            // satisfies &'a ArenaValue<'a> without unsafe.
            if let Some(av) = arena_lit {
                return Ok(av);
            }
            use crate::arena::value_to_arena;
            return Ok(match value {
                Value::Null => crate::arena::pool::singleton_null(),
                Value::Bool(b) => crate::arena::pool::singleton_bool(*b),
                Value::String(s) if s.is_empty() => crate::arena::pool::singleton_empty_string(),
                Value::Array(a) if a.is_empty() => crate::arena::pool::singleton_empty_array(),
                // Borrow the str slice directly from the CompiledNode —
                // no `arena.alloc_str`, no copy. Only safe because `node`
                // is `&'a CompiledNode` and `s` lives at least 'a.
                Value::String(s) => arena.alloc(crate::arena::ArenaValue::String(s.as_str())),
                // Composite literals (Array/Object) — rare. Keep the
                // recursive `value_to_arena` path; their alloc cost
                // dominates over matcher work.
                _ => arena.alloc(value_to_arena(value, arena)),
            });
        }

        // Snapshot context for trace BEFORE recursing — children will
        // mutate iteration frames. Cheap when no tracer is attached.
        #[cfg(feature = "trace")]
        let ctx_snapshot: Option<Value> = actx.has_tracer().then(|| actx.current_data_as_value());

        let result = dispatch::evaluate_arena_node_inner(self, node, actx, arena);

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
        actx: &mut crate::arena::ArenaContextStack<'a>,
        arena: &'a bumpalo::Bump,
        _index: u32,
        _total: u32,
    ) -> Result<&'a crate::arena::ArenaValue<'a>> {
        #[cfg(feature = "trace")]
        actx.trace_push_iteration(_index, _total);
        let res = self.evaluate_arena_node(body, actx, arena);
        #[cfg(feature = "trace")]
        actx.trace_pop_iteration();
        res
    }

    /// Evaluate JSON logic with execution trace for debugging.
    ///
    /// This method compiles and evaluates JSONLogic while recording each
    /// execution step for replay in debugging UIs.
    ///
    /// # Arguments
    ///
    /// * `logic` - JSON logic as a string
    /// * `data` - Data context as a JSON string
    ///
    /// # Returns
    ///
    /// A `TracedResult` containing the result, expression tree, and execution steps.
    ///
    /// # Example
    ///
    /// ```rust
    /// use datalogic_rs::DataLogic;
    ///
    /// let engine = DataLogic::new();
    /// let result = engine.evaluate_json_with_trace(
    ///     r#"{">=": [{"var": "age"}, 18]}"#,
    ///     r#"{"age": 25}"#
    /// ).unwrap();
    ///
    /// println!("Result: {}", result.result);
    /// println!("Steps: {}", result.steps.len());
    /// ```
    #[cfg(feature = "trace")]
    pub fn evaluate_json_with_trace(&self, logic: &str, data: &str) -> Result<TracedResult> {
        let logic_value: Value = serde_json::from_str(logic)?;
        let data_value: Value = serde_json::from_str(data)?;
        let data_arc = Arc::new(data_value);

        // Use compile_for_trace to avoid static evaluation, which would collapse
        // operators into values and eliminate trace steps
        let compiled = Arc::new(CompiledLogic::compile_for_trace(
            &logic_value,
            self.preserve_structure(),
        )?);

        let expression_tree = ExpressionNode::build_from_compiled(&compiled.root);
        let mut collector = TraceCollector::new();
        let (result, error_path) =
            self.evaluate_arena_with_trace(&compiled, data_arc, &mut collector);

        match result {
            Ok(value) => Ok(TracedResult {
                result: value,
                expression_tree,
                steps: collector.into_steps(),
                error: None,
                error_structured: None,
            }),
            Err(e) => {
                let message = e.to_string();
                let mut structured = StructuredError::from(e).with_path(error_path);
                if let Some(name) = compiled.root.operator_name() {
                    structured = structured.with_operator(name);
                }
                Ok(TracedResult {
                    result: Value::Null,
                    expression_tree,
                    steps: collector.into_steps(),
                    error: Some(message),
                    error_structured: Some(structured),
                })
            }
        }
    }

    /// Traced evaluation that returns a [`StructuredError`] on any setup
    /// failure (parse / compile) and embeds a structured error inside
    /// [`TracedResult`] on runtime failure.
    ///
    /// Unlike [`evaluate_json_with_trace`](Self::evaluate_json_with_trace),
    /// this method returns `Err` for parse/compile problems rather than
    /// wrapping them in a trace payload, which matches what non-Rust
    /// consumers usually want (no partial trace data when the rule never
    /// started evaluating).
    #[cfg(feature = "trace")]
    pub fn evaluate_json_with_trace_structured(
        &self,
        logic: &str,
        data: &str,
    ) -> std::result::Result<TracedResult, StructuredError> {
        let logic_value: Value = serde_json::from_str(logic).map_err(Error::from)?;
        let data_value: Value = serde_json::from_str(data).map_err(Error::from)?;
        let data_arc = Arc::new(data_value);

        let compiled = Arc::new(CompiledLogic::compile_for_trace(
            &logic_value,
            self.preserve_structure(),
        )?);

        let expression_tree = ExpressionNode::build_from_compiled(&compiled.root);
        let mut collector = TraceCollector::new();
        let (result, error_path) =
            self.evaluate_arena_with_trace(&compiled, data_arc, &mut collector);

        match result {
            Ok(value) => Ok(TracedResult {
                result: value,
                expression_tree,
                steps: collector.into_steps(),
                error: None,
                error_structured: None,
            }),
            Err(e) => {
                let message = e.to_string();
                let mut structured = StructuredError::from(e).with_path(error_path);
                if let Some(name) = compiled.root.operator_name() {
                    structured = structured.with_operator(name);
                }
                Ok(TracedResult {
                    result: Value::Null,
                    expression_tree,
                    steps: collector.into_steps(),
                    error: Some(message),
                    error_structured: Some(structured),
                })
            }
        }
    }

    /// Arena-mode traced evaluation. Acquires an arena, attaches the
    /// caller's [`TraceCollector`] to the arena context, and dispatches
    /// through [`evaluate_arena_node`]. Returns `(result, error_path)`
    /// where `error_path` is the structured-error breadcrumb of node ids
    /// leading to the failure (empty on success).
    #[cfg(feature = "trace")]
    fn evaluate_arena_with_trace(
        &self,
        compiled: &CompiledLogic,
        data: Arc<Value>,
        collector: &mut TraceCollector,
    ) -> (Result<Value>, Vec<u32>) {
        use crate::arena::{ArenaGuard, arena_to_value};
        let cap = compiled.arena_static_bytes.saturating_mul(2).max(4096);
        let guard = ArenaGuard::acquire(cap);
        let arena = guard.arena();
        let arc_for_borrow = Arc::clone(&data);
        let root_ref: &Value = &arc_for_borrow;
        let mut actx = crate::arena::ArenaContextStack::from_value(root_ref, arena);
        actx.set_tracer(collector);
        let result = self.evaluate_arena_node(&compiled.root, &mut actx, arena);
        match result {
            Ok(av) => {
                let owned = arena_to_value(av);
                let path = actx.take_error_path();
                drop(guard);
                drop(arc_for_borrow);
                drop(data);
                (Ok(owned), path)
            }
            Err(e) => {
                let path = actx.take_error_path();
                (Err(e), path)
            }
        }
    }
}
