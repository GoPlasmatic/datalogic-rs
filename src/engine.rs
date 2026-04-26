use serde_json::Value;
use std::collections::HashMap;
use std::sync::Arc;

use crate::config::EvaluationConfig;

#[cfg(feature = "trace")]
use crate::trace::{ExpressionNode, TraceCollector, TracedResult};
use crate::{CompiledLogic, CompiledNode, ContextStack, Error, Result, StructuredError};

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
    custom_arena_operators: HashMap<String, Box<dyn crate::ArenaOperator>>,
    /// Flag to preserve structure of objects with unknown operators
    #[cfg(feature = "preserve")]
    preserve_structure: bool,
    /// Configuration for evaluation behavior
    config: EvaluationConfig,
}

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
        self.evaluate_via_arena(compiled, data)
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
        self.evaluate_via_arena_ref(compiled, data)
    }

    /// Arena-mode evaluation entry. Acquires a thread-local `Bump` (from the
    /// pool, or freshly sized from the rule's compile-time hint), dispatches
    /// through `evaluate_arena_node`, and converts the result back to owned
    /// `Value` at the boundary. The arena is reset and returned to the pool
    /// when `guard` drops at end of function.
    #[inline]
    fn evaluate_via_arena(&self, compiled: &CompiledLogic, data: Arc<Value>) -> Result<Value> {
        use crate::arena::{ArenaGuard, arena_to_value};
        // Size hint for first-time pool fills: static_bytes × 2, min 4 KiB.
        let cap = compiled.arena_static_bytes.saturating_mul(2).max(4096);
        let guard = ArenaGuard::acquire(cap);
        let arena = guard.arena();
        let arc_for_borrow = Arc::clone(&data);
        let root_ref: &Value = &arc_for_borrow;
        let mut actx = crate::arena::ArenaContextStack::new(root_ref);
        let result = self.evaluate_arena_node(&compiled.root, &mut actx, arena)?;
        let owned = arena_to_value(result);
        drop(guard);
        drop(arc_for_borrow);
        drop(data);
        Ok(owned)
    }

    /// Borrowed-data variant of `evaluate_via_arena`. No Arc::clone — the
    /// caller's `&Value` lives on the caller's stack.
    #[inline]
    fn evaluate_via_arena_ref(&self, compiled: &CompiledLogic, data: &Value) -> Result<Value> {
        use crate::arena::{ArenaGuard, arena_to_value};
        let cap = compiled.arena_static_bytes.saturating_mul(2).max(4096);
        let guard = ArenaGuard::acquire(cap);
        let arena = guard.arena();
        let mut actx = crate::arena::ArenaContextStack::new(data);
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
        let arc_for_borrow = Arc::clone(&data);
        let root_ref: &Value = &arc_for_borrow;
        let mut actx = crate::arena::ArenaContextStack::new(root_ref);
        let result = self.evaluate_arena_node(&compiled.root, &mut actx, arena);
        match result {
            Ok(av) => {
                let owned = arena_to_value(av);
                drop(guard);
                drop(arc_for_borrow);
                drop(data);
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

    /// Compatibility shim — value-mode evaluation via arena dispatch.
    ///
    /// Used by the legacy `evaluate_var` / `evaluate_val` / `evaluate_exists`
    /// helpers that are still reachable from arena raw `var`/`val`/`exists`
    /// forms (rare dynamic-path expressions). The actx is seeded with the
    /// caller's current frame as root so `var` lookups inside the dispatch
    /// see what the calling op sees. A follow-up will port those raw forms
    /// to native arena impls and let us drop this shim.
    #[inline]
    pub(crate) fn evaluate_node(
        &self,
        node: &CompiledNode,
        context: &mut ContextStack,
    ) -> Result<Value> {
        let arena = bumpalo::Bump::new();
        let root_owned: Value = context.current().data().clone();
        let mut actx = crate::arena::ArenaContextStack::new(&root_owned);
        let av = self.evaluate_arena_node(node, &mut actx, &arena)?;
        Ok(crate::arena::arena_to_value(av))
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
        node: &CompiledNode,
        actx: &mut crate::arena::ArenaContextStack<'a>,
        arena: &'a bumpalo::Bump,
    ) -> Result<&'a crate::arena::ArenaValue<'a>> {
        // Literal fast path — no breadcrumb push, no trace step.
        if let CompiledNode::Value { value, .. } = node {
            use crate::arena::value_to_arena;
            return Ok(match value {
                Value::Null => crate::arena::pool::singleton_null(),
                Value::Bool(b) => crate::arena::pool::singleton_bool(*b),
                Value::String(s) if s.is_empty() => crate::arena::pool::singleton_empty_string(),
                Value::Array(a) if a.is_empty() => crate::arena::pool::singleton_empty_array(),
                _ => arena.alloc(value_to_arena(value, arena)),
            });
        }

        // Snapshot context for trace BEFORE recursing — children will
        // mutate iteration frames. Cheap when no tracer is attached.
        #[cfg(feature = "trace")]
        let ctx_snapshot: Option<Value> = actx.has_tracer().then(|| actx.current_data_as_value());

        let result = self.evaluate_arena_node_inner(node, actx, arena);

        // Accumulate the failing node's id on every Err. The legacy
        // path gated this on `M::TRACK_PATH` for plain-mode DCE; arena
        // dispatch always pays the (single) Vec::push since errors are
        // rare and structured-error consumers need the path.
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
        body: &CompiledNode,
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

    /// Inner dispatch — never called directly; reachable only via
    /// [`evaluate_arena_node`] which handles the literal fast path,
    /// breadcrumb accumulation, and trace recording.
    #[inline]
    fn evaluate_arena_node_inner<'a>(
        &self,
        node: &CompiledNode,
        actx: &mut crate::arena::ArenaContextStack<'a>,
        arena: &'a bumpalo::Bump,
    ) -> Result<&'a crate::arena::ArenaValue<'a>> {
        use crate::arena::{ArenaValue, value_to_arena};

        match node {
            // Compiled var: full dispatch via the arena helper. Root-scope
            // hits return `InputRef` (no allocation); frame-data lookups
            // currently clone via `value_to_arena` until Phase 5's
            // ArenaContextStack migration changes frame storage.
            CompiledNode::CompiledVar {
                scope_level,
                segments,
                reduce_hint,
                metadata_hint,
                default_value,
                ..
            } => crate::operators::variable::evaluate_compiled_var_arena(
                crate::operators::variable::CompiledVarSpec {
                    scope_level: *scope_level,
                    segments,
                    reduce_hint: *reduce_hint,
                    metadata_hint: *metadata_hint,
                    default_value: default_value.as_deref(),
                },
                actx,
                self,
                arena,
            ),

            // Compiled exists: full dispatch — root scope walks the input
            // directly, others bridge to the value-mode helper. Result is
            // always a Bool singleton.
            #[cfg(feature = "ext-control")]
            CompiledNode::CompiledExists(data) => {
                crate::operators::variable::evaluate_compiled_exists_arena(
                    data.scope_level,
                    &data.segments,
                    actx,
                    arena,
                )
            }

            // Value literal: handled by the outer `evaluate_arena_node`
            // wrapper before reaching this match.
            CompiledNode::Value { .. } => unreachable!("literal handled by wrapper"),

            // Raw var/val/exists operator forms (rare — most are precompiled
            // to CompiledVar/CompiledExists, but dynamic-path forms remain
            // as BuiltinOperator).
            CompiledNode::BuiltinOperator {
                opcode: crate::OpCode::Var,
                args,
                ..
            } => crate::operators::variable::evaluate_var_arena(args, actx, self, arena),
            #[cfg(feature = "ext-control")]
            CompiledNode::BuiltinOperator {
                opcode: crate::OpCode::Val,
                args,
                ..
            } => crate::operators::variable::evaluate_val_arena(args, actx, self, arena),
            #[cfg(feature = "ext-control")]
            CompiledNode::BuiltinOperator {
                opcode: crate::OpCode::Exists,
                args,
                ..
            } => crate::operators::variable::evaluate_exists_arena(args, actx, self, arena),

            CompiledNode::BuiltinOperator {
                opcode: crate::OpCode::Filter,
                args,
                ..
            } => crate::operators::array::evaluate_filter_arena(args, actx, self, arena),

            CompiledNode::BuiltinOperator {
                opcode: crate::OpCode::Map,
                args,
                ..
            } => crate::operators::array::evaluate_map_arena(args, actx, self, arena),

            CompiledNode::BuiltinOperator {
                opcode: crate::OpCode::All,
                args,
                ..
            } => crate::operators::array::evaluate_all_arena(args, actx, self, arena),

            CompiledNode::BuiltinOperator {
                opcode: crate::OpCode::Some,
                args,
                ..
            } => crate::operators::array::evaluate_some_arena(args, actx, self, arena),

            CompiledNode::BuiltinOperator {
                opcode: crate::OpCode::None,
                args,
                ..
            } => crate::operators::array::evaluate_none_arena(args, actx, self, arena),

            CompiledNode::BuiltinOperator {
                opcode: crate::OpCode::Reduce,
                args,
                ..
            } => crate::operators::array::evaluate_reduce_arena(args, actx, self, arena),

            CompiledNode::BuiltinOperator {
                opcode: crate::OpCode::Merge,
                args,
                ..
            } => crate::operators::array::evaluate_merge_arena(args, actx, self, arena),

            CompiledNode::BuiltinOperator {
                opcode: crate::OpCode::Missing,
                args,
                ..
            } => crate::operators::missing::evaluate_missing_arena(args, actx, self, arena),
            CompiledNode::BuiltinOperator {
                opcode: crate::OpCode::MissingSome,
                args,
                ..
            } => crate::operators::missing::evaluate_missing_some_arena(args, actx, self, arena),

            #[cfg(feature = "ext-string")]
            CompiledNode::BuiltinOperator {
                opcode: crate::OpCode::Length,
                args,
                ..
            } => crate::operators::array::evaluate_length_arena(args, actx, self, arena),

            #[cfg(feature = "ext-array")]
            CompiledNode::BuiltinOperator {
                opcode: crate::OpCode::Sort,
                args,
                ..
            } => crate::operators::array::evaluate_sort_arena(args, actx, self, arena),

            CompiledNode::BuiltinOperator {
                opcode: crate::OpCode::Max,
                args,
                ..
            } => crate::operators::arithmetic::evaluate_max_arena(args, actx, self, arena),
            CompiledNode::BuiltinOperator {
                opcode: crate::OpCode::Min,
                args,
                ..
            } => crate::operators::arithmetic::evaluate_min_arena(args, actx, self, arena),
            CompiledNode::BuiltinOperator {
                opcode: crate::OpCode::Add,
                args,
                ..
            } => crate::operators::arithmetic::evaluate_add_arena(args, actx, self, arena),
            CompiledNode::BuiltinOperator {
                opcode: crate::OpCode::Multiply,
                args,
                ..
            } => crate::operators::arithmetic::evaluate_multiply_arena(args, actx, self, arena),

            // Comparison
            CompiledNode::BuiltinOperator {
                opcode: crate::OpCode::Equals,
                args,
                ..
            } => crate::operators::comparison::evaluate_equals_arena(args, actx, self, arena),
            CompiledNode::BuiltinOperator {
                opcode: crate::OpCode::StrictEquals,
                args,
                ..
            } => {
                crate::operators::comparison::evaluate_strict_equals_arena(args, actx, self, arena)
            }
            CompiledNode::BuiltinOperator {
                opcode: crate::OpCode::NotEquals,
                args,
                ..
            } => crate::operators::comparison::evaluate_not_equals_arena(args, actx, self, arena),
            CompiledNode::BuiltinOperator {
                opcode: crate::OpCode::StrictNotEquals,
                args,
                ..
            } => crate::operators::comparison::evaluate_strict_not_equals_arena(
                args, actx, self, arena,
            ),
            CompiledNode::BuiltinOperator {
                opcode: crate::OpCode::GreaterThan,
                args,
                ..
            } => crate::operators::comparison::evaluate_greater_than_arena(args, actx, self, arena),
            CompiledNode::BuiltinOperator {
                opcode: crate::OpCode::GreaterThanEqual,
                args,
                ..
            } => crate::operators::comparison::evaluate_greater_than_equal_arena(
                args, actx, self, arena,
            ),
            CompiledNode::BuiltinOperator {
                opcode: crate::OpCode::LessThan,
                args,
                ..
            } => crate::operators::comparison::evaluate_less_than_arena(args, actx, self, arena),
            CompiledNode::BuiltinOperator {
                opcode: crate::OpCode::LessThanEqual,
                args,
                ..
            } => crate::operators::comparison::evaluate_less_than_equal_arena(
                args, actx, self, arena,
            ),

            // Logical
            CompiledNode::BuiltinOperator {
                opcode: crate::OpCode::Not,
                args,
                ..
            } => crate::operators::logical::evaluate_not_arena(args, actx, self, arena),
            CompiledNode::BuiltinOperator {
                opcode: crate::OpCode::DoubleNot,
                args,
                ..
            } => crate::operators::logical::evaluate_double_not_arena(args, actx, self, arena),
            CompiledNode::BuiltinOperator {
                opcode: crate::OpCode::And,
                args,
                ..
            } => crate::operators::logical::evaluate_and_arena(args, actx, self, arena),
            CompiledNode::BuiltinOperator {
                opcode: crate::OpCode::Or,
                args,
                ..
            } => crate::operators::logical::evaluate_or_arena(args, actx, self, arena),

            // Control
            CompiledNode::BuiltinOperator {
                opcode: crate::OpCode::If,
                args,
                ..
            } => crate::operators::control::evaluate_if_arena(args, actx, self, arena),
            CompiledNode::BuiltinOperator {
                opcode: crate::OpCode::Ternary,
                args,
                ..
            } => crate::operators::control::evaluate_ternary_arena(args, actx, self, arena),
            #[cfg(feature = "ext-control")]
            CompiledNode::BuiltinOperator {
                opcode: crate::OpCode::Coalesce,
                args,
                ..
            } => crate::operators::control::evaluate_coalesce_arena(args, actx, self, arena),
            #[cfg(feature = "ext-control")]
            CompiledNode::BuiltinOperator {
                opcode: crate::OpCode::Switch,
                args,
                ..
            } => crate::operators::control::evaluate_switch_arena(args, actx, self, arena),

            // Arithmetic binary forms
            CompiledNode::BuiltinOperator {
                opcode: crate::OpCode::Subtract,
                args,
                ..
            } => crate::operators::arithmetic::evaluate_subtract_arena(args, actx, self, arena),
            CompiledNode::BuiltinOperator {
                opcode: crate::OpCode::Divide,
                args,
                ..
            } => crate::operators::arithmetic::evaluate_divide_arena(args, actx, self, arena),
            CompiledNode::BuiltinOperator {
                opcode: crate::OpCode::Modulo,
                args,
                ..
            } => crate::operators::arithmetic::evaluate_modulo_arena(args, actx, self, arena),

            // Math (unary)
            #[cfg(feature = "ext-math")]
            CompiledNode::BuiltinOperator {
                opcode: crate::OpCode::Abs,
                args,
                ..
            } => crate::operators::arithmetic::evaluate_abs_arena(args, actx, self, arena),
            #[cfg(feature = "ext-math")]
            CompiledNode::BuiltinOperator {
                opcode: crate::OpCode::Ceil,
                args,
                ..
            } => crate::operators::arithmetic::evaluate_ceil_arena(args, actx, self, arena),
            #[cfg(feature = "ext-math")]
            CompiledNode::BuiltinOperator {
                opcode: crate::OpCode::Floor,
                args,
                ..
            } => crate::operators::arithmetic::evaluate_floor_arena(args, actx, self, arena),

            // String
            CompiledNode::BuiltinOperator {
                opcode: crate::OpCode::Cat,
                args,
                ..
            } => crate::operators::string::evaluate_cat_arena(args, actx, self, arena),
            CompiledNode::BuiltinOperator {
                opcode: crate::OpCode::Substr,
                args,
                ..
            } => crate::operators::string::evaluate_substr_arena(args, actx, self, arena),
            CompiledNode::BuiltinOperator {
                opcode: crate::OpCode::In,
                args,
                ..
            } => crate::operators::string::evaluate_in_arena(args, actx, self, arena),
            #[cfg(feature = "ext-string")]
            CompiledNode::BuiltinOperator {
                opcode: crate::OpCode::StartsWith,
                args,
                ..
            } => crate::operators::string::evaluate_starts_with_arena(args, actx, self, arena),
            #[cfg(feature = "ext-string")]
            CompiledNode::BuiltinOperator {
                opcode: crate::OpCode::EndsWith,
                args,
                ..
            } => crate::operators::string::evaluate_ends_with_arena(args, actx, self, arena),
            #[cfg(feature = "ext-string")]
            CompiledNode::BuiltinOperator {
                opcode: crate::OpCode::Upper,
                args,
                ..
            } => crate::operators::string::evaluate_upper_arena(args, actx, self, arena),
            #[cfg(feature = "ext-string")]
            CompiledNode::BuiltinOperator {
                opcode: crate::OpCode::Lower,
                args,
                ..
            } => crate::operators::string::evaluate_lower_arena(args, actx, self, arena),
            #[cfg(feature = "ext-string")]
            CompiledNode::BuiltinOperator {
                opcode: crate::OpCode::Trim,
                args,
                ..
            } => crate::operators::string::evaluate_trim_arena(args, actx, self, arena),
            #[cfg(feature = "ext-string")]
            CompiledNode::BuiltinOperator {
                opcode: crate::OpCode::Split,
                args,
                ..
            } => crate::operators::string::evaluate_split_arena(args, actx, self, arena),

            // DateTime
            #[cfg(feature = "datetime")]
            CompiledNode::BuiltinOperator {
                opcode: crate::OpCode::Datetime,
                args,
                ..
            } => crate::operators::datetime::evaluate_datetime_arena(args, actx, self, arena),
            #[cfg(feature = "datetime")]
            CompiledNode::BuiltinOperator {
                opcode: crate::OpCode::Timestamp,
                args,
                ..
            } => crate::operators::datetime::evaluate_timestamp_arena(args, actx, self, arena),
            #[cfg(feature = "datetime")]
            CompiledNode::BuiltinOperator {
                opcode: crate::OpCode::ParseDate,
                args,
                ..
            } => crate::operators::datetime::evaluate_parse_date_arena(args, actx, self, arena),
            #[cfg(feature = "datetime")]
            CompiledNode::BuiltinOperator {
                opcode: crate::OpCode::FormatDate,
                args,
                ..
            } => crate::operators::datetime::evaluate_format_date_arena(args, actx, self, arena),
            #[cfg(feature = "datetime")]
            CompiledNode::BuiltinOperator {
                opcode: crate::OpCode::DateDiff,
                args,
                ..
            } => crate::operators::datetime::evaluate_date_diff_arena(args, actx, self, arena),
            #[cfg(feature = "datetime")]
            CompiledNode::BuiltinOperator {
                opcode: crate::OpCode::Now,
                args,
                ..
            } => crate::operators::datetime::evaluate_now_arena(args, actx, self, arena),

            // Type
            #[cfg(feature = "ext-control")]
            CompiledNode::BuiltinOperator {
                opcode: crate::OpCode::Type,
                args,
                ..
            } => crate::operators::type_op::evaluate_type_arena(args, actx, self, arena),

            // Throw / Try
            #[cfg(feature = "error-handling")]
            CompiledNode::BuiltinOperator {
                opcode: crate::OpCode::Throw,
                args,
                ..
            } => crate::operators::throw::evaluate_throw_arena(args, actx, self, arena),
            #[cfg(feature = "error-handling")]
            CompiledNode::BuiltinOperator {
                opcode: crate::OpCode::Try,
                args,
                ..
            } => crate::operators::try_op::evaluate_try_arena(args, actx, self, arena),

            // Preserve
            #[cfg(feature = "preserve")]
            CompiledNode::BuiltinOperator {
                opcode: crate::OpCode::Preserve,
                args,
                ..
            } => crate::operators::preserve::evaluate_preserve_arena(args, actx, self, arena),

            // Slice
            #[cfg(feature = "ext-array")]
            CompiledNode::BuiltinOperator {
                opcode: crate::OpCode::Slice,
                args,
                ..
            } => crate::operators::array::evaluate_slice_arena(args, actx, self, arena),

            // CompiledThrow — constant-folded error literal. Return Err
            // directly without going through value-mode dispatch.
            #[cfg(feature = "error-handling")]
            CompiledNode::CompiledThrow(data) => Err(Error::Thrown(data.error.clone())),

            // StructuredObject (preserve mode): build the object directly
            // in the arena. Each field's value is evaluated through arena
            // dispatch and stored as `(&'a str, ArenaValue<'a>)` pair.
            #[cfg(feature = "preserve")]
            CompiledNode::StructuredObject(data) => {
                let mut pairs: bumpalo::collections::Vec<'a, (&'a str, ArenaValue<'a>)> =
                    bumpalo::collections::Vec::with_capacity_in(data.fields.len(), arena);
                for (key, n) in data.fields.iter() {
                    let val_av = self.evaluate_arena_node(n, actx, arena)?;
                    let val_owned = match val_av {
                        ArenaValue::InputRef(v) => value_to_arena(v, arena),
                        _ => crate::arena::value::reborrow_arena_value(val_av),
                    };
                    let k_arena: &'a str = arena.alloc_str(key);
                    pairs.push((k_arena, val_owned));
                }
                Ok(arena.alloc(ArenaValue::Object(pairs.into_bump_slice())))
            }

            // Array literal: evaluate each element in arena and build an
            // arena-resident Array. Avoids the value-mode round-trip for
            // [1, {var:"x"}, ...] style nodes.
            CompiledNode::Array { nodes, .. } => {
                let mut items: bumpalo::collections::Vec<'a, ArenaValue<'a>> =
                    bumpalo::collections::Vec::with_capacity_in(nodes.len(), arena);
                for n in nodes.iter() {
                    let av = self.evaluate_arena_node(n, actx, arena)?;
                    items.push(crate::arena::value::reborrow_arena_value(av));
                }
                Ok(arena.alloc(ArenaValue::Array(items.into_bump_slice())))
            }

            // Custom operator: pre-evaluate each arg via arena dispatch
            // (so var lookups borrow into input data) and dispatch through
            // `ArenaOperator`. Args reach the operator as
            // `&'a ArenaValue<'a>` — no `serde_json::Value` round-trip.
            CompiledNode::CustomOperator(data) => {
                let arena_op = self
                    .custom_arena_operators
                    .get(&data.name)
                    .ok_or_else(|| Error::InvalidOperator(data.name.clone()))?;
                let mut arena_args: bumpalo::collections::Vec<'a, &'a ArenaValue<'a>> =
                    bumpalo::collections::Vec::with_capacity_in(data.args.len(), arena);
                for arg in data.args.iter() {
                    arena_args.push(self.evaluate_arena_node(arg, actx, arena)?);
                }
                arena_op.evaluate_arena(&arena_args, actx, arena)
            }

            // CompiledSplitRegex (ext-string regex split): build the result
            // object directly in the arena.
            #[cfg(feature = "ext-string")]
            CompiledNode::CompiledSplitRegex(data) => {
                crate::operators::string::evaluate_split_with_regex_arena(
                    &data.args,
                    actx,
                    self,
                    &data.regex,
                    &data.capture_names,
                    arena,
                )
            }

            // No fallback — every CompiledNode shape is covered by an
            // explicit arm above. Reaching this branch is a compile-error
            // (missing match arm) for newly-added shapes, not a runtime
            // panic. If a future variant lands and you see this, add the
            // dispatch arm.
            #[allow(unreachable_patterns)]
            _ => Err(Error::InvalidArguments(
                "internal: unhandled CompiledNode shape in arena dispatch".into(),
            )),
        }
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
    /// where `error_path` mirrors the breadcrumb that the legacy
    /// `Structured` mode produced.
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
        let mut actx = crate::arena::ArenaContextStack::new(root_ref);
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
