use std::cell::Cell;
use std::collections::HashMap;

use crate::config::EvaluationConfig;

use crate::{CompiledNode, Logic, Result};

thread_local! {
    /// Per-thread re-entry counter for the `Engine::evaluate` boundary.
    /// Bumped by `enter_dispatch_boundary` on entry and restored by the
    /// `DepthGuard` on drop (not touched by `dispatch_node` itself), so the
    /// value reflects how many nested `Engine::evaluate(...)` calls are
    /// currently live on the sync call stack.
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
/// rules ([`Self::compile`]), evaluating them ([`Self::eval`] /
// `Self::eval_into` is feature-gated on `serde_json`; link it
// conditionally so default-features `cargo doc` doesn't break.
#[cfg_attr(
    feature = "serde_json",
    doc = "[`Self::eval_str`], [`Self::eval_into`]), and opening hot-loop"
)]
#[cfg_attr(
    not(feature = "serde_json"),
    doc = "[`Self::eval_str`], `Self::eval_into`), and opening hot-loop"
)]
/// sessions ([`Self::session`]).
// The `trace` feature adds [`Self::trace`]; reference it conditionally so
// `cargo doc` without `--all-features` doesn't break on the intra-doc link.
#[cfg_attr(
    feature = "trace",
    doc = "Enabling the `trace` feature also exposes [`Self::trace`] for traced sessions."
)]
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
/// // 3. Evaluate against many inputs (here via `Session::eval_str`;
/// //    drop to `Engine::evaluate` if you want zero-copy borrowed results).
/// let mut session = engine.session();
/// for age in [12, 18, 42] {
///     let payload = format!(r#"{{"age": {age}}}"#);
///     let result = session.eval_str(&logic, &payload).unwrap();
///     assert!(result == "\"adult\"" || result == "\"minor\"");
///     session.reset();
/// }
/// ```
///
/// # Choosing an evaluate method
///
/// **Start here.** Use [`Self::eval_str`] for one-shot calls. Switch
/// to [`crate::Session`] once you're evaluating the same compiled rule
/// many times — it reuses one arena instead of allocating per call.
/// Drop down to [`Self::evaluate`] only when you're managing your own
/// `bumpalo::Bump` (custom pools, integration with arena-aware
/// downstream code).
///
/// Result-shape suffixes work the same on every tier: `(none)` returns
/// [`datavalue::OwnedDataValue`], `_str` returns [`String`] (JSON),
/// `_into::<T>` returns `T: DeserializeOwned` (requires `serde_json`).
/// The raw [`Self::evaluate`] is the only method that exposes
/// `&'a DataValue<'a>` and a caller-owned `&Bump`.
///
/// Three tiers, in order of caller control:
///
/// | Method | Arena ownership | Result type | When to use |
/// |---|---|---|---|
// `eval_into` is feature-gated on `serde_json`; emit a linked or plain
// reference depending on the active features so default-features
// `cargo doc` doesn't break the table row.
#[cfg_attr(
    feature = "serde_json",
    doc = "| [`Self::eval`] / [`Self::eval_str`] / [`Self::eval_into`] | engine creates a fresh `Bump::with_capacity(4096)` per call | [`OwnedDataValue`](datavalue::OwnedDataValue) / `String` / `T` | One-shot. Any caller that doesn't want to think about arenas. Allocates each call — for hot loops, drop to `Session`. |"
)]
#[cfg_attr(
    not(feature = "serde_json"),
    doc = "| [`Self::eval`] / [`Self::eval_str`] / `Self::eval_into` | engine creates a fresh `Bump::with_capacity(4096)` per call | [`OwnedDataValue`](datavalue::OwnedDataValue) / `String` / `T` | One-shot. Any caller that doesn't want to think about arenas. Allocates each call — for hot loops, drop to `Session`. |"
)]
#[cfg_attr(
    feature = "serde_json",
    doc = "| [`crate::Session::eval`] / [`crate::Session::eval_str`] / [`crate::Session::eval_into`] / [`crate::Session::eval_borrowed`] | session-owned `Bump`, caller calls [`crate::Session::reset`] between batches | owned / `String` / `T` / borrowed `&'a DataValue<'a>` | Hot loop with a long-lived engine. The `Session` hides `bumpalo` from the call site and pre-sizes the arena via [`crate::Session::reset_with_capacity`] when needed. |"
)]
#[cfg_attr(
    not(feature = "serde_json"),
    doc = "| [`crate::Session::eval`] / [`crate::Session::eval_str`] / `Session::eval_into` / [`crate::Session::eval_borrowed`] | session-owned `Bump`, caller calls [`crate::Session::reset`] between batches | owned / `String` / `T` / borrowed `&'a DataValue<'a>` | Hot loop with a long-lived engine. The `Session` hides `bumpalo` from the call site and pre-sizes the arena via [`crate::Session::reset_with_capacity`] when needed. |"
)]
/// | [`Self::evaluate`] | caller-passed `&Bump`; library never resets | `&'a DataValue<'a>` (borrowed) | Zero-copy result paths, custom pool/allocator strategies, integration with arena-aware downstream code. |
///
/// All routes share the same dispatcher; the differences are who owns
/// the arena, what the result type looks like, and whether the
/// boundary parses / serialises JSON for you. There is no perf
/// difference between the arena-aware paths once the bump is warm.
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
    /// Whether `Engine::compile` runs the constant-folding pass.
    /// Defaults to `true`; toggled via
    /// [`crate::EngineBuilder::with_constant_folding`]. The trace surface
    /// always disables folding regardless of this flag (handled in
    /// `TracedSession`).
    constant_folding: bool,
    /// Configuration for evaluation behavior
    config: EvaluationConfig,
}

mod dispatch;

/// Convert an `OwnedDataValue` literal to an arena-resident `DataValue`
/// reference. Reached from the `dispatch_node` literal path for any
/// `CompiledNode::Value` whose `lit` was not precomputed — in practice
/// only ad-hoc `synthetic_value` wrappers built outside the compile
/// pipeline, since `populate_lits` pre-builds every literal reachable from
/// a `Logic`. Trivial cases (Null, Bool, empty primitives) hit shared
/// singletons with no allocation; a non-empty String allocates a single
/// `DataValue` wrapper into the per-call arena (the `&str` is borrowed from
/// the owned source); non-empty Arrays/Objects rebuild their spine in the
/// arena via [`borrow_to_arena`], borrowing string bytes from the owned
/// source instead of copying them.
///
/// `#[cold]` + `#[inline(never)]`: `dispatch_node` is `#[inline(always)]`,
/// so its literal path is stamped into every operator's dispatch site —
/// keeping this fallback outlined keeps those sites small (I-cache), and
/// the cold hint steers the branch layout toward the pre-built `lit` path
/// that every compiled literal takes.
#[cold]
#[inline(never)]
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
        _ => arena.alloc(borrow_to_arena(value, arena)),
    }
}

/// Convert an owned composite literal into an arena `DataValue`, borrowing
/// string bytes (element strings and object keys) from the owned source
/// instead of copying them into the arena the way
/// `OwnedDataValue::to_arena` does. Only the array/object spine is built in
/// the arena. Sound because the compiled node — and therefore `value` —
/// outlives the evaluation: `dispatch_node` borrows the node at the same
/// `'a` as the arena. Recursion depth mirrors the value's nesting, which is
/// bounded by the JSON parser / compile-time depth caps upstream.
fn borrow_to_arena<'a>(
    value: &'a datavalue::OwnedDataValue,
    arena: &'a bumpalo::Bump,
) -> crate::arena::DataValue<'a> {
    use crate::arena::DataValue;
    use datavalue::OwnedDataValue;
    match value {
        OwnedDataValue::Null => DataValue::Null,
        OwnedDataValue::Bool(b) => DataValue::Bool(*b),
        OwnedDataValue::Number(n) => DataValue::Number(*n),
        OwnedDataValue::String(s) => DataValue::String(s.as_str()),
        OwnedDataValue::Array(items) => DataValue::Array(
            arena.alloc_slice_fill_with(items.len(), |i| borrow_to_arena(&items[i], arena)),
        ),
        OwnedDataValue::Object(pairs) => {
            DataValue::Object(arena.alloc_slice_fill_with(pairs.len(), |i| {
                let (k, v) = &pairs[i];
                (k.as_str(), borrow_to_arena(v, arena))
            }))
        }
        #[cfg(feature = "datetime")]
        OwnedDataValue::DateTime(d) => DataValue::DateTime(*d),
        #[cfg(feature = "datetime")]
        OwnedDataValue::Duration(d) => DataValue::Duration(*d),
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
    /// For a stock engine, [`Self::new`] is shorter.
    #[inline]
    pub fn builder() -> crate::EngineBuilder {
        crate::EngineBuilder::new()
    }

    /// Open a [`crate::Session`] handle that owns a reusable arena and
    /// returns owned results, so callers don't need to manage a
    /// [`bumpalo::Bump`] themselves.
    ///
    /// Use this when you want the throughput of arena reuse without the
    /// lifetime juggling of [`Self::evaluate`]. Results are deep-cloned out
    /// of the arena before returning, so they survive later calls and resets.
    /// The session does **not** auto-reset: allocations accumulate until you
    /// call [`crate::Session::reset`], which you should do between logical
    /// batches to bound peak memory in long-running services.
    ///
    /// # Example
    ///
    /// ```rust
    /// use datalogic_rs::Engine;
    ///
    /// let engine = Engine::new();
    /// let compiled = engine.compile(r#"{"+": [{"var": "x"}, 1]}"#).unwrap();
    /// let mut session = engine.session();
    /// let result = session.eval_str(&compiled, r#"{"x": 41}"#).unwrap();
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
        constant_folding: bool,
        operators: HashMap<String, Box<dyn crate::CustomOperator>>,
    ) -> Self {
        Self {
            custom_operators: operators,
            #[cfg(feature = "templating")]
            templating: _templating,
            constant_folding,
            config,
        }
    }

    /// Creates a new Engine with all built-in operators.
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
        Self::from_builder_parts(EvaluationConfig::default(), false, true, HashMap::new())
    }

    /// Gets a reference to the current evaluation configuration.
    pub fn config(&self) -> &EvaluationConfig {
        &self.config
    }

    /// Internal: whether the constant-folding pass runs during
    /// [`Self::compile`]. Reads the field set by
    /// [`crate::EngineBuilder::with_constant_folding`].
    #[inline]
    pub(crate) fn constant_folding_enabled(&self) -> bool {
        self.constant_folding
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
    // V5 PUBLIC API
    //   - One-shot:   `eval` / `eval_str` / `eval_into`   (engine-owned arena per call)
    //   - Power tier: `evaluate(&Logic, D, &Bump)`        (caller-owned arena, borrowed result)
    //   - Hot loop:   `engine.session().eval*(...)`       (pooled arena, manual reset)
    //   - Trace:      `engine.trace().eval*(...)`         (Session mirror with TracedRun<R>)
    // ============================================================

    /// Compile a rule source into reusable [`Logic`].
    ///
    /// `rule` accepts any [`crate::IntoLogic`] shape: `&str` (JSON-parsed),
    /// `&OwnedDataValue` / `OwnedDataValue` (cloned/moved), or
    /// `&serde_json::Value` (gated on `serde_json`). For cross-thread
    /// sharing prefer [`Self::compile_arc`].
    ///
    /// # Example
    ///
    /// ```rust
    /// use datalogic_rs::Engine;
    ///
    /// let engine = Engine::new();
    /// let compiled = engine.compile(r#"{"==": [{"var": "x"}, 1]}"#).unwrap();
    /// ```
    pub fn compile<R: crate::IntoLogic>(&self, rule: R) -> Result<Logic> {
        let owned = rule.into_owned_logic()?;
        Logic::compile_with(&owned, self)
    }

    /// Compile and wrap in an [`Arc`](std::sync::Arc) in one call. Convenience for the
    /// dominant cross-thread-sharing pattern; equivalent to
    /// `Arc::new(engine.compile(rule)?)`.
    pub fn compile_arc<R: crate::IntoLogic>(&self, rule: R) -> Result<std::sync::Arc<Logic>> {
        Ok(std::sync::Arc::new(self.compile(rule)?))
    }

    /// Open a [`crate::TracedSession`] over this engine. Calls made through
    /// the session collect a per-call trace; the bare `eval*` methods on
    /// `Engine` itself pay no trace overhead.
    ///
    /// Available only when the crate is built with `feature = "trace"`.
    ///
    /// # Trace coverage
    ///
    /// The session's one-shot [`crate::TracedSession::eval_str`] compiles
    /// the rule internally with optimization disabled, so every operator
    /// in the rule surfaces a trace step.
    ///
    /// The pre-compiled paths ([`crate::TracedSession::eval`] taking a
    /// `&Logic`) inherit whatever shape that `Logic` was compiled into —
    /// constant sub-expressions folded by [`Self::compile`] won't appear,
    /// since there is no operator left to execute. Use `eval_str` for
    /// full coverage on a one-shot run.
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
    ///     .eval_str(r#"{"+": [1, 2]}"#, "null");
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

    /// Evaluate compiled logic against arena-resident data — **raw tier**.
    ///
    /// The caller owns the [`bumpalo::Bump`] lifecycle and may `reset()`
    /// it between calls; the returned `&DataValue<'a>` borrows from the
    /// arena, so it must be dropped before the next reset (enforced by
    /// the borrow checker). For ergonomic owned/typed/JSON-string output,
    /// prefer [`Self::eval`] / [`Self::eval_str`]
    // `Self::eval_into` is gated behind `serde_json`; link conditionally.
    #[cfg_attr(feature = "serde_json", doc = "/ [`Self::eval_into`].")]
    #[cfg_attr(
        not(feature = "serde_json"),
        doc = "(plus `Self::eval_into` with the `serde_json` feature)."
    )]
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
    /// `&'a DataValue<'a>` (zero-cost passthrough), `DataValue<'a>`
    /// (single arena alloc), `&str` (JSON-parsed), `&OwnedDataValue`
    /// (deep-borrowed), [`&ParsedData`](crate::ParsedData) (zero-cost
    /// passthrough of a parse-once handle), or `&serde_json::Value`
    /// (gated on `serde_json`).
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

    /// Apply the engine's configured truthiness rules
    /// ([`crate::TruthyEvaluator`]) to an evaluated value.
    ///
    /// This is the same coercion `if` / `and` / `or` / `!` apply to
    /// their operands, exposed so callers (and bindings) can collapse
    /// any rule result to a boolean without re-implementing the
    /// engine's configured semantics.
    ///
    /// # Example
    ///
    /// ```rust
    /// use bumpalo::Bump;
    /// use datalogic_rs::Engine;
    ///
    /// let engine = Engine::new();
    /// let compiled = engine.compile(r#"{"var": "items"}"#).unwrap();
    /// let arena = Bump::new();
    /// let result = engine.evaluate(&compiled, r#"{"items": [1]}"#, &arena).unwrap();
    /// assert!(engine.truthy(result));
    /// ```
    #[inline]
    pub fn truthy(&self, value: &crate::arena::DataValue<'_>) -> bool {
        crate::arena::truthy_arena(value, self)
    }

    /// One-shot evaluation returning [`datavalue::OwnedDataValue`].
    ///
    /// Compiles `rule`, parses `data`, evaluates against a fresh
    /// per-call arena, and deep-clones the result out. For the same
    /// rule run repeatedly, escalate to [`Self::compile`] + a
    /// [`Session`](crate::Session).
    ///
    /// # Example
    ///
    /// ```rust
    /// use datalogic_rs::Engine;
    ///
    /// let engine = Engine::new();
    /// let result = engine.eval(
    ///     r#"{"+": [{"var": "x"}, 1]}"#,
    ///     r#"{"x": 41}"#,
    /// ).unwrap();
    /// assert_eq!(result.as_i64(), Some(42));
    /// ```
    pub fn eval<R, D>(&self, rule: R, data: D) -> Result<datavalue::OwnedDataValue>
    where
        R: crate::IntoLogic,
        D: crate::OwnedInput,
    {
        self.eval_with::<datavalue::OwnedDataValue, _, _>(rule, data)
    }

    /// One-shot evaluation returning a JSON [`String`].
    ///
    /// # Example
    ///
    /// ```rust
    /// use datalogic_rs::Engine;
    ///
    /// let engine = Engine::new();
    /// let result = engine.eval_str(
    ///     r#"{"==": [{"var": "x"}, 5]}"#,
    ///     r#"{"x": 5}"#,
    /// ).unwrap();
    /// assert_eq!(result, "true");
    /// ```
    pub fn eval_str<R, D>(&self, rule: R, data: D) -> Result<String>
    where
        R: crate::IntoLogic,
        D: crate::OwnedInput,
    {
        self.eval_with::<String, _, _>(rule, data)
    }

    /// One-shot evaluation deserialised into a typed `T: DeserializeOwned`.
    ///
    /// Use `T = serde_json::Value` for a JSON `Value` result; use a typed
    /// struct for direct mapping. Internally routes through `serde_json`
    /// (round-trips the result through a JSON value).
    ///
    /// # Example
    ///
    /// ```rust
    /// # #[cfg(feature = "serde_json")] {
    /// use datalogic_rs::Engine;
    /// use serde_json::Value;
    ///
    /// let engine = Engine::new();
    /// let result: Value = engine.eval_into(
    ///     r#"{"+": [{"var": "x"}, 1]}"#,
    ///     r#"{"x": 41}"#,
    /// ).unwrap();
    /// assert_eq!(result, Value::from(42));
    /// # }
    /// ```
    #[cfg(feature = "serde_json")]
    #[cfg_attr(docsrs, doc(cfg(feature = "serde_json")))]
    pub fn eval_into<T, R, D>(&self, rule: R, data: D) -> Result<T>
    where
        T: serde::de::DeserializeOwned,
        R: crate::IntoLogic,
        D: crate::OwnedInput,
    {
        let value: serde_json::Value = self.eval_with(rule, data)?;
        serde_json::from_value(value).map_err(crate::Error::from)
    }

    /// Internal generic shared by `eval` / `eval_str` / `eval_into`.
    /// Compiles, allocates a fresh per-call arena, evaluates, and
    /// projects the result through [`crate::FromDataValue`].
    fn eval_with<O, R, D>(&self, rule: R, data: D) -> Result<O>
    where
        O: crate::FromDataValue,
        R: crate::IntoLogic,
        D: crate::OwnedInput,
    {
        let compiled = self.compile(rule)?;
        // 4 KB initial capacity covers typical small-rule evaluations.
        let arena = bumpalo::Bump::with_capacity(4096);
        let owned_data = data.into_owned_input()?;
        let result = self.evaluate(&compiled, &owned_data, &arena)?;
        O::from_arena(result)
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
            // Every literal reachable from a `Logic` carries a pre-built
            // `PreLit` — trivial ones from `precompute_lit` at node
            // construction, composites from the `populate_lits` pass —
            // so the hot path is a borrow, not a conversion.
            if let Some(av) = lit {
                return Ok(av.as_ref());
            }
            // Synthetic composite wrappers built outside the compile
            // pipeline fall through to per-call arena conversion here.
            return Ok(literal_fallback(value, arena));
        }

        // Snapshot context for trace BEFORE recursing — children will
        // mutate iteration frames. Cheap when no tracer is attached.
        #[cfg(feature = "trace")]
        let ctx_snapshot: Option<serde_json::Value> =
            ctx.has_tracer().then(|| ctx.current_data_as_value());

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
