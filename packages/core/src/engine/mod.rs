#[cfg(feature = "compat")]
use serde_json::Value;
use std::cell::Cell;
use std::collections::HashMap;

use crate::config::EvaluationConfig;

use crate::{CompiledNode, Logic, Result};

#[cfg(feature = "trace")]
mod trace;

thread_local! {
    /// Per-thread `dispatch_node` recursion counter. Incremented at the
    /// top of `dispatch_node` (after the literal fast path) and decremented
    /// on the way out, so the value reflects the current sync call-stack
    /// depth across nested `Engine::evaluate(...)` invocations.
    ///
    /// Why thread-local rather than a `ContextStack` field: a custom
    /// operator can hold `Arc<Engine>` and call `engine.evaluate(...)`
    /// recursively from inside its own `evaluate(...)` — each top-level
    /// call constructs a fresh `ContextStack` (depth resets to 0) but
    /// the C call stack keeps growing. A thread-local survives across
    /// those boundaries and catches the runaway recursion before stack
    /// overflow.
    ///
    /// Tokio safety: `dispatch_node` is sync, so a task can't `.await`
    /// while holding the counter raised. Between dispatch calls the
    /// value returns to zero, so cross-thread task migration starts
    /// fresh on whatever thread it lands.
    static DISPATCH_DEPTH: Cell<u32> = const { Cell::new(0) };
}

/// Restores [`DISPATCH_DEPTH`] to its prior value on drop. Used by
/// the boundary entry points (`Engine::evaluate`, `TracedSession::evaluate`)
/// so early returns and panics leave the counter consistent.
///
/// `DepthGuard(u32::MAX)` is a no-op sentinel — used when the engine has
/// no custom operators registered, so cross-evaluate recursion is
/// impossible and the boundary skips the TLS bookkeeping entirely. The
/// drop check makes the guard zero-cost in that case.
pub(crate) struct DepthGuard(u32);

impl DepthGuard {
    const NOOP: u32 = u32::MAX;
}

impl Drop for DepthGuard {
    #[inline]
    fn drop(&mut self) {
        if self.0 != Self::NOOP {
            DISPATCH_DEPTH.with(|d| d.set(self.0));
        }
    }
}

/// JSONLogic compile/evaluate engine.
///
/// Holds the immutable engine state — registered [`crate::CustomOperator`]
/// implementations, the [`EvaluationConfig`], the optional
/// preserve-structure flag — and exposes the public surface for parsing
/// rules ([`Self::compile`]), evaluating them ([`Self::evaluate`],
/// [`Self::evaluate_str`], [`Self::evaluate_json_value`]), and opening
/// hot-loop / traced sessions ([`Self::session`], [`Self::trace`]).
///
/// `Engine` is `Send + Sync` (every field is); the typical pattern is to
/// build one at startup, wrap it in `Arc<Engine>`, and clone the `Arc`
/// across threads or async tasks.
///
/// # Example
///
/// ```rust
/// use datalogic_rs::Engine;
///
/// // 1. Build the engine.
/// let engine = Engine::new();
///
/// // 2. Compile a rule once.
/// let logic = engine
///     .compile(r#"{"if": [{">=": [{"var": "age"}, 18]}, "adult", "minor"]}"#)
///     .unwrap();
///
/// // 3. Evaluate against many inputs (here via the convenience `evaluate_str`;
/// //    use `Session::evaluate*` or `Engine::evaluate` for hot-loop variants).
/// let mut session = engine.session();
/// for age in [12, 18, 42] {
///     let payload = format!(r#"{{"age": {age}}}"#);
///     let result = session.evaluate_str(&logic, &payload).unwrap();
///     assert!(result == "\"adult\"" || result == "\"minor\"");
///     session.reset();
/// }
/// ```
///
/// # Choosing an evaluate method
///
/// **Start here.** Use [`Self::evaluate_str`] for one-shot calls. Switch
/// to [`crate::Session`] once you're evaluating the same compiled rule
/// many times — it reuses one arena instead of allocating per call. Drop
/// down to [`Self::evaluate`] only when you're managing your own
/// `bumpalo::Bump` (custom pools, integration with arena-aware downstream
/// code). The table below is the full comparison; the three-line summary
/// above covers the dominant choice.
///
/// Three tiers, in order of caller control:
///
/// | Method | Arena ownership | Result type | When to use |
/// |---|---|---|---|
/// | [`Self::evaluate_str`] | engine creates a fresh `Bump::with_capacity(4096)` per call | `String` (JSON) | One-shot. CLI scripts, "I want JSON in and JSON out", any caller that doesn't want to think about arenas. Allocates each call — for hot loops, drop to `Session`. |
/// | [`crate::Session::evaluate`] / [`crate::Session::evaluate_ref`] / [`crate::Session::evaluate_str`] | session-owned `Bump`, caller calls [`crate::Session::reset`] between batches | owned (`OwnedDataValue` / `String`) or borrowed `&'a DataValue<'a>` | Hot loop with a long-lived engine. The `Session` hides `bumpalo` from the call site and pre-sizes the arena via [`crate::Session::reset_with_capacity`] when needed. |
/// | [`Self::evaluate`] | caller-passed `&Bump`; library never resets | `&'a DataValue<'a>` (borrowed) | Zero-copy result paths, custom pool/allocator strategies, integration with arena-aware downstream code. |
/// | [`Self::evaluate_json_value`] (gated on `compat`) | engine creates a fresh `Bump::with_capacity(4096)` per call | `serde_json::Value` | Drop-in for v4 callers and any path that needs the `serde_json::Value` boundary on both sides. |
///
/// All four route through the same dispatcher; the only differences are
/// who owns the arena, what the result type looks like, and whether the
/// boundary parses / serialises JSON for you. There is no perf
/// difference between the arena-aware paths once the bump is warm —
/// pick the one whose ergonomics fit your call site.
///
/// See the crate-level docs for the two-phase architecture, threading
/// model, and walk-through examples; see the `Session` and
/// `EvaluationConfig` rustdoc for arena-management and behaviour-tuning
/// options respectively.
pub struct Engine {
    /// Custom `CustomOperator` implementations registered with the engine.
    pub(super) custom_operators: HashMap<String, Box<dyn crate::CustomOperator>>,
    /// Whether templating mode is enabled — multi-key objects compile
    /// to output-shaping templates and unknown operator keys pass through.
    #[cfg(feature = "templating")]
    templating: bool,
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

impl std::fmt::Debug for Engine {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        // Print the operator *count* rather than names: names are user
        // registration data, and `Engine::custom_operator_names()` exposes
        // them already for callers who want them. The trait objects
        // themselves can't render a meaningful Debug.
        let mut s = f.debug_struct("Engine");
        s.field("custom_operators", &self.custom_operators.len());
        #[cfg(feature = "templating")]
        s.field("templating", &self.templating);
        s.field("config", &self.config);
        s.finish_non_exhaustive()
    }
}

impl Engine {
    /// Start a [`crate::EngineBuilder`] for fluent construction.
    ///
    /// Use the builder when you need a non-default [`EvaluationConfig`],
    /// templating mode, or pre-registered custom operators.
    /// For a stock engine, [`Self::new`] is shorter. The 4.x
    /// `with_preserve_structure` / `with_config` / `with_config_and_structure`
    /// constructors are still reachable through `compat::LegacyApi` when the
    /// crate is built with `feature = "compat"`
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
        _templating: bool,
        operators: HashMap<String, Box<dyn crate::CustomOperator>>,
    ) -> Self {
        Self {
            custom_operators: operators,
            #[cfg(feature = "templating")]
            templating: _templating,
            config,
        }
    }

    /// Creates a new Engine engine with all built-in operators.
    ///
    /// The engine includes 50+ built-in operators optimized with OpCode dispatch.
    /// Templating mode is disabled by default. For non-default
    /// configuration (custom [`EvaluationConfig`], templating mode,
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

    /// Internal: whether templating mode is on. Always returns `false`
    /// when the crate is built without `feature = "templating"` (the
    /// underlying field doesn't exist off-feature). Folded here so the
    /// single call site in `compile/` doesn't repeat the `#[cfg]` ceremony.
    #[inline]
    pub(crate) fn is_templating_enabled(&self) -> bool {
        #[cfg(feature = "templating")]
        {
            self.templating
        }
        #[cfg(not(feature = "templating"))]
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

    /// Iterator over the names of every *custom* operator registered on
    /// this engine (built-ins are not included). Order is unspecified
    /// (HashMap iteration order). Useful for tooling, UIs, and tests
    /// that need to introspect what's available.
    pub fn custom_operator_names(&self) -> impl Iterator<Item = &str> {
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
    /// The session's one-shot [`crate::TracedSession::evaluate_str`] compiles
    /// the rule internally with optimization disabled, so every operator in the
    /// rule surfaces a trace step.
    ///
    /// The pre-compiled paths ([`crate::TracedSession::evaluate`] taking a `&Logic`)
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
    ///     .trace()
    ///     .evaluate_str(r#"{"+": [1, 2]}"#, "null");
    /// assert_eq!(run.result.unwrap(), "3");
    /// // run.steps is the per-node execution log;
    /// // run.expression_tree is the rule's compile-time tree shape.
    /// # }
    /// ```
    #[cfg(feature = "trace")]
    #[cfg_attr(docsrs, doc(cfg(feature = "trace")))]
    #[inline]
    pub fn trace(&self) -> crate::trace::TracedSession<'_> {
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
        let _depth_guard = self.enter_dispatch_boundary()?;
        let data_ref = data.into_arena_value(arena)?;
        let mut ctx = crate::arena::ContextStack::new(data_ref);
        match self.dispatch_node(&compiled.root, &mut ctx, arena) {
            Ok(av) => Ok(av),
            Err(e) => Err(e.decorated(ctx.take_error_path(), compiled, true)),
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
        // 4 KB initial capacity covers typical small-rule evaluations without
        // bumpalo's first-chunk grow path. Larger inputs grow as usual.
        let arena = bumpalo::Bump::with_capacity(4096);
        let data_dv = datavalue::DataValue::from_str(data, &arena)?;
        let result = self.evaluate(&compiled, data_dv, &arena)?;
        Ok(crate::arena::data_to_json_string(result))
    }

    /// One-shot evaluation with `serde_json::Value` boundary on both sides.
    ///
    /// Mirror of [`Self::evaluate_str`] for callers already on `serde_json`.
    /// Funnels through [`Self::evaluate`] internally.
    #[cfg(feature = "compat")]
    #[cfg_attr(docsrs, doc(cfg(feature = "compat")))]
    pub fn evaluate_json_value(&self, logic: &Value, data: &Value) -> Result<Value> {
        let logic_owned = crate::compat::owned_from_serde(logic);
        let compiled = Logic::compile_with(&logic_owned, self)?;
        self.run_to_value(&compiled, data)
    }

    /// Internal `&Value -> Value` adapter shared by [`Self::evaluate_json_value`]
    /// and the v4 compat shims in [`crate::compat::LegacyApi`]. Routes
    /// through the public [`Self::evaluate`] so the dispatch path is
    /// identical regardless of entry point.
    #[cfg(feature = "compat")]
    pub(crate) fn run_to_value(&self, compiled: &Logic, data: &Value) -> Result<Value> {
        let arena = bumpalo::Bump::with_capacity(4096);
        let data_av = crate::arena::value_to_data(data, &arena);
        let result = self.evaluate(compiled, data_av, &arena)?;
        Ok(crate::arena::data_to_value(result))
    }

    /// Bump the per-thread dispatch-boundary depth counter, bailing with
    /// `ConfigurationError` if the configured cap is reached. Returns a
    /// guard that decrements the counter on drop (covers `?` early returns
    /// and panics).
    ///
    /// Called from every public boundary entry point (`Engine::evaluate`,
    /// `TracedSession::evaluate`, …). The counter is thread-local rather
    /// than per-`ContextStack` so it survives across nested
    /// `engine.evaluate(...)` calls — the scenario a `CustomOperator`
    /// holding `Arc<Engine>` creates by re-entering the engine from inside
    /// its own `evaluate(...)`.
    ///
    /// Tokio safety: dispatch is sync, so a task cannot `.await` while the
    /// counter is raised; between dispatches the value is restored to its
    /// prior level (zero at the outermost call). Cross-thread task migration
    /// thus starts fresh on whatever thread the task lands on.
    #[inline(always)]
    pub(crate) fn enter_dispatch_boundary(&self) -> Result<DepthGuard> {
        // Built-in operators can't re-enter `Engine::evaluate` (only a
        // `CustomOperator` holding `Arc<Engine>` can); when the registry
        // is empty, cross-evaluate recursion is impossible and we skip
        // the TLS bookkeeping. The pure-built-in benchmarks pay zero.
        if self.custom_operators.is_empty() {
            return Ok(DepthGuard(DepthGuard::NOOP));
        }
        self.enter_dispatch_boundary_checked()
    }

    /// Slow path of [`Self::enter_dispatch_boundary`] — hit only when
    /// the engine has at least one custom operator registered. Marked
    /// `#[cold]` and `#[inline(never)]` so the hot fast-path stays
    /// inline-friendly.
    #[cold]
    #[inline(never)]
    fn enter_dispatch_boundary_checked(&self) -> Result<DepthGuard> {
        let prev_depth = DISPATCH_DEPTH.with(Cell::get);
        if prev_depth >= self.config.max_recursion_depth {
            return Err(crate::Error::configuration_error(format!(
                "max recursion depth exceeded ({})",
                self.config.max_recursion_depth
            )));
        }
        DISPATCH_DEPTH.with(|d| d.set(prev_depth + 1));
        Ok(DepthGuard(prev_depth))
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
}
