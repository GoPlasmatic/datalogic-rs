#[cfg(feature = "compat")]
use serde_json::Value;
use std::collections::HashMap;
#[cfg(feature = "trace")]
use std::sync::Arc;

use crate::config::EvaluationConfig;

#[cfg(feature = "trace")]
use crate::trace::{ExpressionNode, TraceCollector, TracedResult};
use crate::{CompiledNode, Logic, Result};

/// JSONLogic compile/evaluate engine. See the crate-level docs for the
/// two-phase architecture, threading model, and walk-through examples.
pub struct Engine {
    /// Custom `CustomOperator` implementations registered with the engine.
    pub(super) custom_operators: HashMap<String, Box<dyn crate::CustomOperator>>,
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
/// Convert an `OwnedDataValue` to an arena-resident `DataValue` reference.
/// Trivial cases (Null, Bool, empties) hit shared singletons with no
/// allocation; non-empty Strings allocate a single `DataValue` wrapper into
/// the per-call arena (the `&str` is borrowed from the owned source);
/// non-empty Arrays/Objects deep-convert via `value.to_arena`.
#[inline]
fn literal_fallback<'a>(
    value: &'a datavalue::OwnedDataValue,
    arena: &'a bumpalo::Bump,
) -> &'a crate::arena::DataValue<'a> {
    use datavalue::OwnedDataValue;
    match value {
        OwnedDataValue::Null => crate::arena::singletons::singleton_null(),
        OwnedDataValue::Bool(b) => crate::arena::singletons::singleton_bool(*b),
        OwnedDataValue::String(s) if s.is_empty() => {
            crate::arena::singletons::singleton_empty_string()
        }
        OwnedDataValue::Array(a) if a.is_empty() => {
            crate::arena::singletons::singleton_empty_array()
        }
        OwnedDataValue::Object(o) if o.is_empty() => {
            crate::arena::singletons::singleton_empty_object()
        }
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
    /// Use the builder when you need a non-default [`EvaluationConfig`],
    /// structure-preservation mode, or pre-registered custom operators.
    /// For a stock engine, [`Self::new`] is shorter. The 4.x
    /// `with_preserve_structure` / `with_config` / `with_config_and_structure`
    /// constructors are still reachable through [`crate::compat::LegacyApi`]
    /// (`use datalogic_rs::compat::LegacyApi;`).
    #[inline]
    pub fn builder() -> crate::EngineBuilder {
        crate::EngineBuilder::new()
    }

    /// Open a [`crate::Session`] handle that owns a reusable arena and
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
    /// let mut session = engine.session();
    /// let result = session.evaluate_str(&compiled, r#"{"x": 41}"#).unwrap();
    /// assert_eq!(result, "42");
    /// ```
    #[inline]
    pub fn session(&self) -> crate::Session<'_> {
        crate::Session::new(self)
    }

    /// Internal seam used by the builder. `pub(crate)` is enough — no
    /// `#[doc(hidden)]` needed since it's not externally reachable.
    #[inline]
    pub(crate) fn from_builder_parts(
        config: EvaluationConfig,
        _preserve_structure: bool,
        operators: HashMap<String, Box<dyn crate::CustomOperator>>,
    ) -> Self {
        Self {
            custom_operators: operators,
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
        Self::from_builder_parts(EvaluationConfig::default(), false, HashMap::new())
    }

    /// Gets a reference to the current evaluation configuration.
    pub fn config(&self) -> &EvaluationConfig {
        &self.config
    }

    /// Returns whether structure preservation is enabled. Always returns
    /// `false` when the crate is built without `feature = "preserve"`
    /// (the underlying field doesn't exist off-feature).
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

    /// Checks if a custom operator with the given name is registered.
    ///
    /// Operator registration is builder-only; this is a read-only check
    /// against the frozen set produced by [`crate::EngineBuilder`].
    pub fn has_custom_operator(&self, name: &str) -> bool {
        self.custom_operators.contains_key(name)
    }

    /// Iterator over the names of every custom operator registered on this
    /// engine. Order is unspecified (HashMap iteration order). Useful for
    /// tooling, UIs, and tests that need to introspect what's available.
    pub fn operator_names(&self) -> impl Iterator<Item = &str> {
        self.custom_operators.keys().map(String::as_str)
    }

    // ============================================================
    // V5 PUBLIC API — power users compile once + evaluate many; normal
    // users call `evaluate_str` directly.
    // ============================================================

    /// Compile a JSON logic string into reusable [`Logic`].
    ///
    /// The canonical v5 entry point for compilation. Returns an owned
    /// [`Logic`] that can be reused across many evaluations on a single
    /// thread. For cross-thread sharing wrap the result yourself:
    /// `Arc::new(engine.compile(logic)?)` — `Arc<Logic>` derefs transparently
    /// into `&Logic` for [`Self::evaluate`] /
    /// [`Session::evaluate`](crate::Session::evaluate).
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
        Logic::compile_with(&owned, self)
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
    /// `data` accepts any input shape understood by [`crate::EvalInput`]:
    /// `&'a DataValue<'a>` (zero-cost passthrough), `DataValue<'a>` (single
    /// arena alloc), `&str` (JSON-parsed), `&OwnedDataValue`
    /// (deep-borrowed), or `&serde_json::Value` (deep-converted, requires
    /// the `compat` feature).
    #[inline(always)]
    pub fn evaluate<'a, D: crate::EvalInput<'a>>(
        &self,
        compiled: &'a Logic,
        data: D,
        arena: &'a bumpalo::Bump,
    ) -> Result<&'a crate::arena::DataValue<'a>> {
        let data_ref = data.into_arena_value(arena)?;
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
    pub fn evaluate_serde(&self, logic: &Value, data: &Value) -> Result<Value> {
        let logic_owned = crate::compat::owned_from_serde(logic);
        let compiled = Logic::compile_with(&logic_owned, self)?;
        self.run_to_value(&compiled, data)
    }

    /// Internal `&Value -> Value` adapter shared by [`Self::evaluate_serde`]
    /// and the v4 compat shims in [`crate::compat::LegacyApi`]. Routes
    /// through the public [`Self::evaluate`] so the dispatch path is
    /// identical regardless of entry point.
    #[cfg(feature = "compat")]
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
            // Trivial literals (Null/Bool/Number/empty primitives) are
            // pre-built `DataValue<'static>` by `precompute_lit` at node
            // construction; covariance lets `&'a DataValue<'static>`
            // satisfy `&'a DataValue<'a>` directly.
            if let Some(av) = lit {
                return Ok(av);
            }
            // Non-trivial literals (non-empty Strings/Arrays/Objects) and
            // synthetic nodes built outside the compile pipeline fall
            // through to per-call arena allocation here.
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
        // Move the caller's collector into ctx, leaving a fresh empty
        // collector in its place; restore the populated one back to the
        // caller's slot after dispatch.
        let owned = std::mem::take(collector);
        ctx.attach_tracer(owned);
        let result = self.dispatch_node(&compiled.root, &mut ctx, &arena);
        *collector = ctx.detach_tracer().expect("attach_tracer was called above");
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
