#[cfg(feature = "compat")]
use serde_json::Value;
use std::collections::HashMap;
#[cfg(feature = "trace")]
use std::sync::Arc;

use crate::config::EvaluationConfig;

#[cfg(feature = "trace")]
use crate::trace::{ExpressionNode, TraceCollector, TracedResult};
use crate::{CompiledNode, Logic, Result};

/// The main Engine engine for compiling and evaluating JSONLogic expressions.
///
/// The engine provides a two-phase approach to logic evaluation:
/// 1. **Compilation**: Parse JSON logic into optimized `Logic`
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
/// use datalogic_rs::Engine;
///
/// let engine = Engine::new();
/// let result = engine.evaluate_str(
///     r#"{">": [{"var": "age"}, 18]}"#,
///     r#"{"age": 21}"#,
/// ).unwrap();
/// assert_eq!(result, "true");
/// ```
pub struct Engine {
    /// Custom `Operator` implementations registered with the engine.
    pub(super) custom_operators: HashMap<String, Box<dyn crate::Operator>>,
    /// Flag to preserve structure of objects with unknown operators
    #[cfg(feature = "preserve")]
    preserve_structure: bool,
    /// Configuration for evaluation behavior
    config: EvaluationConfig,
}

mod dispatch;

/// Cold fallback for `CompiledNode::Value { lit: None, .. }` — only
/// reached by ad-hoc `synthetic_value` wrappers (test helpers, trace nodes
/// built outside `Logic::new`). Outlined so the inliner doesn't
/// expand it into the hot `dispatch_node` literal arm.
#[cold]
#[inline(never)]
fn literal_fallback<'a>(
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

impl Default for Engine {
    fn default() -> Self {
        Self::new()
    }
}

impl Engine {
    /// Start a [`crate::EngineBuilder`] for fluent construction.
    ///
    /// Replaces the 4.x `new` / `with_preserve_structure` / `with_config` /
    /// `with_config_and_structure` constructors. The old methods are still
    /// reachable through [`crate::compat::LegacyApi`] — bring the trait into
    /// scope (`use datalogic_rs::compat::LegacyApi;`) to opt into the legacy
    /// surface.
    #[inline]
    pub fn builder() -> crate::EngineBuilder {
        crate::EngineBuilder::new()
    }

    /// Open a [`crate::Scratch`] handle that owns a reusable arena and
    /// returns owned results, so callers don't need to manage a
    /// [`bumpalo::Bump`] themselves.
    ///
    /// Use this when you want the throughput of arena reuse without the
    /// lifetime juggling of [`Self::evaluate`]. The arena resets at the start
    /// of every `eval*` call; results are deep-cloned out of the arena
    /// before returning, so they outlive the next reset.
    ///
    /// # Example
    ///
    /// ```rust
    /// use datalogic_rs::Engine;
    ///
    /// let engine = Engine::new();
    /// let compiled = engine.compile(r#"{"+": [{"var": "x"}, 1]}"#).unwrap();
    /// let mut scratch = engine.scratch();
    /// let result = scratch.eval_str(&compiled, r#"{"x": 41}"#).unwrap();
    /// assert_eq!(result, "42");
    /// ```
    #[inline]
    pub fn scratch(&self) -> crate::Scratch<'_> {
        crate::Scratch::new(self)
    }

    /// Internal seam used by the builder. Not part of the public API.
    #[doc(hidden)]
    #[inline]
    pub(crate) fn from_builder_parts(
        config: EvaluationConfig,
        _preserve_structure: bool,
        operators: HashMap<String, Box<dyn crate::Operator>>,
    ) -> Self {
        Self {
            custom_operators: operators,
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
            custom_operators: HashMap::new(),
            #[cfg(feature = "preserve")]
            preserve_structure: _preserve_structure,
            config,
        }
    }

    /// Creates a new Engine engine with all built-in operators.
    ///
    /// The engine includes 50+ built-in operators optimized with OpCode dispatch.
    /// Structure preservation is disabled by default. For non-default
    /// configuration (custom [`EvaluationConfig`], structure preservation,
    /// pre-registered custom operators) prefer [`Self::builder`].
    ///
    /// # Example
    ///
    /// ```rust
    /// use datalogic_rs::Engine;
    ///
    /// let engine = Engine::new();
    /// ```
    pub fn new() -> Self {
        Self::new_inner(EvaluationConfig::default(), false)
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

    /// Registers a custom [`crate::Operator`] with the engine.
    ///
    /// Implementations take pre-evaluated args as `&'a DataValue<'a>` and
    /// return an arena-allocated result.
    ///
    /// # Example
    ///
    /// ```rust
    /// use datalogic_rs::{ContextStack, Operator, DataValue, Engine, Result};
    /// use bumpalo::Bump;
    ///
    /// struct Plus42;
    /// impl Operator for Plus42 {
    ///     fn evaluate<'a>(
    ///         &self,
    ///         args: &[&'a DataValue<'a>],
    ///         _ctx: &mut ContextStack<'a>,
    ///         arena: &'a Bump,
    ///     ) -> Result<&'a DataValue<'a>> {
    ///         let n = args.first().and_then(|v| v.as_f64()).unwrap_or(0.0);
    ///         Ok(arena.alloc(DataValue::from_f64(n + 42.0)))
    ///     }
    /// }
    ///
    /// let mut engine = Engine::new();
    /// engine.add_operator("plus42", Plus42);
    /// let result = engine.evaluate_str(r#"{"plus42": 8}"#, "null").unwrap();
    /// assert_eq!(result, "50");
    /// ```
    pub fn add_operator(&mut self, name: impl Into<String>, operator: impl crate::IntoOperatorBox) {
        self.custom_operators
            .insert(name.into(), operator.into_operator_box());
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
        self.custom_operators.contains_key(name)
    }

    /// Iterator over the names of every custom operator currently registered
    /// with this engine. Order is unspecified (HashMap iteration order).
    /// Useful for tooling, UIs, and tests that need to introspect what's
    /// available.
    pub fn operator_names(&self) -> impl Iterator<Item = &str> {
        self.custom_operators.keys().map(String::as_str)
    }

    /// Remove a custom operator by name. Returns the removed operator's
    /// boxed handle if it was registered, `None` otherwise. Built-in
    /// operators dispatched via [`crate::OpCode`] are not affected.
    pub fn remove_operator(&mut self, name: &str) -> Option<Box<dyn crate::Operator>> {
        self.custom_operators.remove(name)
    }

    // ============================================================
    // V5 PUBLIC API — power users compile once + evaluate many; normal
    // users call `evaluate_str` directly.
    // ============================================================

    /// Compile a JSON logic string into reusable [`Logic`].
    ///
    /// The canonical v5 entry point for compilation. Returns an owned
    /// [`Logic`] that can be reused across many evaluations on a
    /// single thread. For cross-thread sharing use [`Self::compile_arc`]
    /// instead — it wraps the result in [`std::sync::Arc`] so clones are
    /// `O(1)`.
    ///
    /// # Example
    ///
    /// ```rust
    /// use datalogic_rs::Engine;
    ///
    /// let engine = Engine::new();
    /// let compiled = engine.compile(r#"{"==": [{"var": "x"}, 1]}"#).unwrap();
    /// ```
    pub fn compile(&self, logic: &str) -> Result<Logic> {
        let owned = datavalue::OwnedDataValue::from_json(logic)?;
        self.compile_value(&owned)
    }

    /// Compile a JSON logic string and wrap it in [`std::sync::Arc`] for
    /// cross-thread sharing.
    ///
    /// Equivalent to `Arc::new(engine.compile(logic)?)` but saves the
    /// boilerplate at every call site. The returned `Arc<Logic>`
    /// derefs transparently into `&Logic`, so it slots into
    /// [`Self::evaluate`] / [`Scratch::eval`](crate::Scratch::eval) without
    /// any additional adaptation.
    ///
    /// # Example
    ///
    /// ```rust
    /// use std::sync::Arc;
    /// use std::thread;
    /// use datalogic_rs::Engine;
    ///
    /// let engine = Engine::new();
    /// let compiled = engine.compile_arc(r#"{">": [{"var": "score"}, 90]}"#).unwrap();
    ///
    /// let handles: Vec<_> = (0..3)
    ///     .map(|i| {
    ///         let compiled = Arc::clone(&compiled);
    ///         thread::spawn(move || {
    ///             let engine = Engine::new();
    ///             let payload = format!(r#"{{"score": {}}}"#, 80 + i * 5);
    ///             engine.scratch().eval_str(&compiled, &payload).unwrap()
    ///         })
    ///     })
    ///     .collect();
    /// for h in handles { let _ = h.join(); }
    /// ```
    #[inline]
    pub fn compile_arc(&self, logic: &str) -> Result<std::sync::Arc<Logic>> {
        Ok(std::sync::Arc::new(self.compile(logic)?))
    }

    /// Internal compile helper shared by [`Self::compile`] and the compat
    /// `compile_serde_value` shim. Not part of the public API.
    #[doc(hidden)]
    pub(crate) fn compile_value(&self, logic: &datavalue::OwnedDataValue) -> Result<Logic> {
        Logic::compile_with(logic, self)
    }

    /// Open a [`crate::TracedSession`] over this engine. Calls made through
    /// the session collect a per-call trace; the bare `evaluate*` methods on
    /// `Engine` itself are unchanged and pay no trace overhead.
    ///
    /// Available only when the crate is built with `feature = "trace"`.
    ///
    /// # Trace coverage
    ///
    /// The session's one-shot [`TracedSession::evaluate_str`] compiles the
    /// rule internally with optimization disabled, so every operator in the
    /// rule surfaces a trace step.
    ///
    /// The pre-compiled paths ([`TracedSession::evaluate`] taking a `&Logic`)
    /// inherit whatever shape that `Logic` was compiled into — constant
    /// sub-expressions folded by [`Self::compile`] won't appear, since there
    /// is no operator left to execute. Use `evaluate_str` for full coverage
    /// on a one-shot run.
    ///
    /// # Example
    ///
    /// ```rust
    /// # #[cfg(feature = "trace")] {
    /// use datalogic_rs::Engine;
    ///
    /// let engine = Engine::new();
    /// let run = engine
    ///     .with_trace()
    ///     .evaluate_str(r#"{"+": [1, 2]}"#, "null");
    /// assert_eq!(run.result.unwrap(), "3");
    /// // run.steps is the per-node execution log;
    /// // run.expression_tree is the rule's compile-time tree shape.
    /// # }
    /// ```
    #[cfg(feature = "trace")]
    #[inline]
    pub fn with_trace(&self) -> crate::trace::TracedSession<'_> {
        crate::trace::TracedSession::new(self)
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
    /// use datalogic_rs::{Engine, DataValue};
    ///
    /// let engine = Engine::new();
    /// let compiled = engine.compile(r#"{"+": [{"var": "x"}, 2]}"#).unwrap();
    ///
    /// let arena = Bump::new();
    /// let data = DataValue::from_str(r#"{"x": 40}"#, &arena).unwrap();
    /// let result = engine.evaluate(&compiled, data, &arena).unwrap();
    /// assert_eq!(result.as_i64(), Some(42));
    /// ```
    ///
    /// `data` accepts any input shape understood by [`crate::IntoEvalData`]:
    /// `&'a DataValue<'a>` (zero-cost passthrough), `DataValue<'a>` (single
    /// arena alloc), `&str` (JSON-parsed), `&OwnedDataValue`
    /// (deep-borrowed), or `&serde_json::Value` (deep-converted, requires
    /// the `compat` feature).
    #[inline(always)]
    pub fn evaluate<'a, D: crate::IntoEvalData<'a>>(
        &self,
        compiled: &'a Logic,
        data: D,
        arena: &'a bumpalo::Bump,
    ) -> Result<&'a crate::arena::DataValue<'a>> {
        let data_ref = data.into_eval_data(arena)?;
        let mut ctx = crate::arena::ContextStack::new(data_ref);
        match self.dispatch_node(&compiled.root, &mut ctx, arena) {
            Ok(av) => Ok(av),
            Err(mut e) => {
                e = e.with_path(ctx.take_error_path());
                if let Some(name) = compiled.root.operator_name() {
                    e = e.with_operator(name);
                }
                Err(e)
            }
        }
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
    /// use datalogic_rs::Engine;
    ///
    /// let engine = Engine::new();
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
        let result = self.evaluate(&compiled, data_dv, &arena)?;
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
        let data_av = crate::arena::value_to_data(data, &arena);
        let result = self.evaluate(&compiled, data_av, &arena)?;
        Ok(crate::arena::data_to_value(result))
    }

    /// Internal `&Value -> Value` adapter used by the compat shims in
    /// [`crate::compat::LegacyApi`]. Routes through the public
    /// [`Self::evaluate`] so the dispatch path is identical to the v5 entry.
    #[cfg(feature = "compat")]
    #[doc(hidden)]
    pub(crate) fn run_to_value(&self, compiled: &Logic, data: &Value) -> Result<Value> {
        let arena = bumpalo::Bump::new();
        let data_av = crate::arena::value_to_data(data, &arena);
        let result = self.evaluate(compiled, data_av, &arena)?;
        Ok(crate::arena::data_to_value(result))
    }

    /// Arena-mode dispatch hub. Returns `&'a DataValue<'a>` for every
    /// `CompiledNode` shape — exhaustive match, no value-mode fallback.
    ///
    /// On error, accumulates the failing node's id onto the context stack's
    /// breadcrumb so [`Error`] consumers can surface the failing
    /// path. When a tracer is attached to `ctx`, records a step per
    /// non-literal node (entry context + result/error).
    #[inline(always)]
    pub(crate) fn dispatch_node<'a>(
        &self,
        node: &'a CompiledNode,
        ctx: &mut crate::arena::ContextStack<'a>,
        arena: &'a bumpalo::Bump,
    ) -> Result<&'a crate::arena::DataValue<'a>> {
        // Literal fast path — no breadcrumb push, no trace step.
        if let CompiledNode::Value { value, lit, .. } = node {
            // Compiled-tree literals always have `lit` populated by
            // `populate_lits` (run during `Logic::new`), so
            // this branch covers every literal in any finalized rule.
            // DataValue is covariant in its lifetime, so
            // `&'a DataValue<'static>` satisfies `&'a DataValue<'a>`
            // without unsafe.
            if let Some(av) = lit {
                return Ok(av);
            }
            // Fallback for nodes built outside the compile pipeline (test
            // helpers in `trace.rs`, ad-hoc `synthetic_value` wrappers that
            // never went through `Logic::new`). Outlined + cold so
            // the literal fast path stays a single load+branch in the
            // dispatched dominant case.
            return Ok(literal_fallback(value, arena));
        }

        // Snapshot context for trace BEFORE recursing — children will
        // mutate iteration frames. Cheap when no tracer is attached.
        #[cfg(feature = "trace")]
        let ctx_snapshot: Option<Value> = ctx.has_tracer().then(|| ctx.current_data_as_value());

        let result = dispatch::dispatch_node_inner(self, node, ctx, arena);

        // Accumulate the failing node's id on every Err. We always pay
        // the (single) Vec::push since errors are rare and structured-error
        // consumers need the breadcrumb.
        if result.is_err() {
            ctx.push_error_step(node.id());
        }

        #[cfg(feature = "trace")]
        if let Some(ctx_data) = ctx_snapshot {
            ctx.record_node_result(node.id(), ctx_data, &result);
        }

        result
    }

    /// Evaluate an iteration body (map/filter/reduce/all/some/none) with the
    /// trace collector's iteration index/total markers set around it. The
    /// markers are no-ops when no tracer is attached, so plain-mode callers
    /// pay only one branch per iteration.
    #[inline]
    pub(crate) fn run_iter_body<'a>(
        &self,
        body: &'a CompiledNode,
        ctx: &mut crate::arena::ContextStack<'a>,
        arena: &'a bumpalo::Bump,
        _index: u32,
        _total: u32,
    ) -> Result<&'a crate::arena::DataValue<'a>> {
        #[cfg(feature = "trace")]
        ctx.trace_push_iteration(_index, _total);
        let res = self.dispatch_node(body, ctx, arena);
        #[cfg(feature = "trace")]
        ctx.trace_pop_iteration();
        res
    }

    /// Run a traced evaluation and assemble the [`TracedResult`]. Used by the
    /// [`crate::compat::LegacyApi`] trace shims.
    #[cfg(feature = "trace")]
    #[doc(hidden)]
    pub(crate) fn run_trace(&self, compiled: &Logic, data_arc: Arc<Value>) -> TracedResult {
        let expression_tree = ExpressionNode::build_from_compiled(&compiled.root);
        let mut collector = TraceCollector::new();
        let (result, error_path) = self.run_with_trace(compiled, data_arc, &mut collector);
        let steps = collector.into_steps();
        match result {
            Ok(value) => TracedResult {
                result: value,
                expression_tree,
                steps,
                error: None,
                error_structured: None,
            },
            Err(mut e) => {
                let message = e.to_string();
                e = e.with_path(error_path);
                if let Some(name) = compiled.root.operator_name() {
                    e = e.with_operator(name);
                }
                TracedResult {
                    result: Value::Null,
                    expression_tree,
                    steps,
                    error: Some(message),
                    error_structured: Some(e),
                }
            }
        }
    }

    /// Arena-mode traced evaluation. Allocates an arena, attaches the
    /// caller's [`TraceCollector`] to the arena context, and dispatches
    /// through [`dispatch_node`]. Returns `(result, error_path)` where
    /// `error_path` is the structured-error breadcrumb of node ids leading
    /// to the failure (empty on success). Calls `dispatch_node` directly
    /// (not the public [`Self::evaluate`]) because the trace path needs the
    /// [`crate::arena::ContextStack`] both before (to attach the tracer)
    /// and after (to extract the breadcrumb) the evaluation.
    #[cfg(feature = "trace")]
    fn run_with_trace(
        &self,
        compiled: &Logic,
        data: Arc<Value>,
        collector: &mut TraceCollector,
    ) -> (Result<Value>, Vec<u32>) {
        let arena = bumpalo::Bump::new();
        let data_av = crate::arena::value_to_data(&data, &arena);
        let mut ctx = crate::arena::ContextStack::new(arena.alloc(data_av));
        ctx.set_tracer(collector);
        let result = self.dispatch_node(&compiled.root, &mut ctx, &arena);
        match result {
            Ok(av) => {
                let owned = crate::arena::data_to_value(av);
                let path = ctx.take_error_path();
                (Ok(owned), path)
            }
            Err(e) => {
                let path = ctx.take_error_path();
                (Err(e), path)
            }
        }
    }
}
