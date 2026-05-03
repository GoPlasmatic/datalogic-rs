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
        OwnedDataValue::String(s) if s.is_empty() => {
            crate::arena::pool::singleton_empty_string()
        }
        OwnedDataValue::Array(a) if a.is_empty() => {
            crate::arena::pool::singleton_empty_array()
        }
        OwnedDataValue::String(s) => {
            arena.alloc(crate::arena::DataValue::String(s.as_str()))
        }
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
    /// use serde_json::{json, Value};
    /// use std::sync::Arc;
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
    /// use serde_json::json;
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
    /// let compiled = engine.compile(&json!({"plus42": 8})).unwrap();
    /// assert_eq!(engine.evaluate_ref(&compiled, &json!({})).unwrap(), json!(50));
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
    #[cfg(feature = "compat")]
    pub fn compile(&self, logic: &Value) -> Result<Arc<CompiledLogic>> {
        let owned = crate::value::owned_from_serde(logic);
        self.compile_value(&owned)
    }

    /// Compile a parsed [`OwnedDataValue`] rule into reusable
    /// [`CompiledLogic`]. The hot-path entry — no `serde_json::Value` round
    /// trip.
    pub fn compile_value(&self, logic: &datavalue::OwnedDataValue) -> Result<Arc<CompiledLogic>> {
        let compiled = CompiledLogic::compile_with_static_eval(logic, self)?;
        Ok(Arc::new(compiled))
    }

    /// Compile a raw JSON string into reusable [`CompiledLogic`]. Parses
    /// with the `datavalue` parser (no `serde_json` dependency).
    pub fn compile_str(&self, logic: &str) -> Result<Arc<CompiledLogic>> {
        let owned = datavalue::OwnedDataValue::from_json(logic)?;
        self.compile_value(&owned)
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
    #[cfg(feature = "compat")]
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
    #[cfg(feature = "compat")]
    pub fn evaluate_ref(&self, compiled: &CompiledLogic, data: &Value) -> Result<Value> {
        self.eval_to_value(compiled, data)
    }

    /// Arena-mode evaluation entry. Acquires a thread-local `Bump` (from the
    /// pool, or freshly sized from the rule's compile-time hint), dispatches
    /// through `evaluate_node`, and converts the result back to owned
    /// `Value` at the boundary. The arena is reset and returned to the pool
    /// when `guard` drops at end of function.
    #[cfg(feature = "compat")]
    #[inline]
    fn eval_to_value(&self, compiled: &CompiledLogic, data: &Value) -> Result<Value> {
        use crate::arena::{ArenaGuard, arena_to_value};
        let guard = ArenaGuard::acquire(compiled.arena_capacity());
        let arena = guard.arena();
        let mut actx = crate::arena::DataContextStack::from_value(data, arena);
        let result = self.evaluate_node(&compiled.root, &mut actx, arena)?;
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
    #[cfg(feature = "compat")]
    pub fn evaluate_owned(&self, compiled: &CompiledLogic, data: Value) -> Result<Value> {
        self.evaluate(compiled, Arc::new(data))
    }

    // (evaluate_ref is the canonical zero-Arc path; see definition above.)

    /// Pure arena evaluation against a caller-provided [`bumpalo::Bump`].
    /// Returns an arena-allocated `&DataValue<'a>` — the v5 hot-path entry.
    ///
    /// The caller owns the `Bump` lifecycle and decides when to `reset()` it;
    /// the returned reference borrows from `arena`, so it must be dropped
    /// before the next `arena.reset()` (the borrow checker enforces this).
    ///
    /// Pre-parse JSON input via
    /// `OwnedDataValue::from_json(s)?.to_arena(&arena)` (or
    /// `DataValue::from_str(s, &arena)?` from `datavalue`) to obtain the
    /// `&DataValue<'a>` data argument.
    ///
    /// ```ignore
    /// use bumpalo::Bump;
    /// use datalogic_rs::DataLogic;
    ///
    /// let engine = DataLogic::new();
    /// let compiled = engine.compile_str(r#"{"+": [1, 2]}"#).unwrap();
    /// let arena = Bump::new();
    /// let data = datavalue::DataValue::from_str("null", &arena).unwrap();
    /// let result = engine.evaluate_value(&compiled, arena.alloc(data), &arena).unwrap();
    /// assert_eq!(result.as_i64(), Some(3));
    /// ```
    #[inline(always)]
    pub fn evaluate_value<'a>(
        &self,
        compiled: &'a CompiledLogic,
        data: &'a crate::arena::DataValue<'a>,
        arena: &'a bumpalo::Bump,
    ) -> Result<&'a crate::arena::DataValue<'a>> {
        let mut actx = crate::arena::DataContextStack::new(data);
        self.evaluate_node(&compiled.root, &mut actx, arena)
    }

    /// Deprecated v4-era alias for [`Self::evaluate_value`]. Kept while the
    /// benchmark binary references it; new code should call `evaluate_value`.
    #[doc(hidden)]
    #[inline(always)]
    pub fn evaluate_in_arena<'a>(
        &self,
        compiled: &'a CompiledLogic,
        data: &'a crate::arena::DataValue<'a>,
        arena: &'a bumpalo::Bump,
    ) -> Result<&'a crate::arena::DataValue<'a>> {
        self.evaluate_value(compiled, data, arena)
    }

    /// One-shot string boundary: parse `logic` + `data` JSON, evaluate
    /// against `compiled`, return the result as a JSON `String`. Acquires
    /// a fresh per-call [`bumpalo::Bump`] internally; for repeated calls,
    /// drive [`Self::evaluate_value`] with a reused arena instead.
    ///
    /// Pure v5 entry — no `serde_json` round-trips. Replaces the legacy
    /// `evaluate_json` (which returned a `serde_json::Value` and is now
    /// gated behind the `compat` feature).
    pub fn evaluate_str(&self, compiled: &CompiledLogic, data: &str) -> Result<String> {
        let arena = bumpalo::Bump::new();
        let data_dv = datavalue::DataValue::from_str(data, &arena)?;
        let data_ref = arena.alloc(data_dv);
        let result = self.evaluate_value(compiled, data_ref, &arena)?;
        Ok(crate::arena::data_to_json_string(result))
    }

    /// Convenience: compile the logic from a string, evaluate against the
    /// data string, return the result as a JSON `String`. Use
    /// [`Self::compile_str`] + [`Self::evaluate_str`] separately when the
    /// same logic runs against many data inputs.
    pub fn evaluate_logic_str(&self, logic: &str, data: &str) -> Result<String> {
        let compiled = self.compile_str(logic)?;
        self.evaluate_str(&compiled, data)
    }

    /// Pure arena evaluation for benchmarking — runs `evaluate_node`
    /// against an internally-acquired `ArenaGuard`. Kept as the equivalent
    /// of the public `evaluate*` API minus the `arena_to_value` boundary,
    /// so callers can compare dispatch-only cost with vs. without the
    /// thread-local arena slot. Not part of the stable API.
    #[cfg(feature = "compat")]
    #[doc(hidden)]
    pub fn evaluate_bench(&self, compiled: &CompiledLogic, data: &Value) -> Result<()> {
        use crate::arena::ArenaGuard;
        let guard = ArenaGuard::acquire(compiled.arena_capacity());
        let arena = guard.arena();
        let mut actx = crate::arena::DataContextStack::from_value(data, arena);
        let result = self.evaluate_node(&compiled.root, &mut actx, arena)?;
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
    #[cfg(feature = "compat")]
    pub fn evaluate_json(&self, logic: &str, data: &str) -> Result<Value> {
        let (compiled, data_arc) = self.parse_and_compile(logic, data)?;
        self.evaluate(&compiled, data_arc)
    }

    /// Parse `logic` and `data` JSON strings, compile the logic, and wrap
    /// the data in an `Arc`. Shared boilerplate for the public `evaluate_json*`
    /// entry points.
    #[cfg(feature = "compat")]
    fn parse_and_compile(
        &self,
        logic: &str,
        data: &str,
    ) -> Result<(Arc<CompiledLogic>, Arc<Value>)> {
        let logic_value: Value = serde_json::from_str(logic)?;
        let data_value: Value = serde_json::from_str(data)?;
        let compiled = self.compile(&logic_value)?;
        Ok((compiled, Arc::new(data_value)))
    }

    /// Evaluates a compiled rule, returning a `StructuredError` on failure.
    ///
    /// Identical to [`evaluate`](Self::evaluate) on success. On error, the
    /// `Error` is wrapped with the name of the outermost operator in the
    /// compiled logic, so non-Rust consumers can surface typed error
    /// information without parsing `Display` strings.
    #[cfg(feature = "compat")]
    pub fn evaluate_structured(
        &self,
        compiled: &CompiledLogic,
        data: Arc<Value>,
    ) -> std::result::Result<Value, StructuredError> {
        use crate::arena::{ArenaGuard, arena_to_value};
        let guard = ArenaGuard::acquire(compiled.arena_capacity());
        let arena = guard.arena();
        let mut actx = crate::arena::DataContextStack::from_value(&data, arena);
        match self.evaluate_node(&compiled.root, &mut actx, arena) {
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
    #[cfg(feature = "compat")]
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
        let compiled = self.compile_for_trace(&logic_value)?;
        Ok(self.run_trace(&compiled, data_arc))
    }

    /// Arena-mode traced evaluation. Acquires an arena, attaches the
    /// caller's [`TraceCollector`] to the arena context, and dispatches
    /// through [`evaluate_node`]. Returns `(result, error_path)`
    /// where `error_path` is the structured-error breadcrumb of node ids
    /// leading to the failure (empty on success).
    #[cfg(feature = "trace")]
    fn evaluate_with_trace(
        &self,
        compiled: &CompiledLogic,
        data: Arc<Value>,
        collector: &mut TraceCollector,
    ) -> (Result<Value>, Vec<u32>) {
        use crate::arena::{ArenaGuard, arena_to_value};
        let guard = ArenaGuard::acquire(compiled.arena_capacity());
        let arena = guard.arena();
        let arc_for_borrow = Arc::clone(&data);
        let root_ref: &Value = &arc_for_borrow;
        let mut actx = crate::arena::DataContextStack::from_value(root_ref, arena);
        actx.set_tracer(collector);
        let result = self.evaluate_node(&compiled.root, &mut actx, arena);
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
