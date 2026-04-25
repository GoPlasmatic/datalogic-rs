use serde_json::Value;
use std::borrow::Cow;
use std::collections::HashMap;
use std::sync::Arc;

use crate::config::EvaluationConfig;

// The arena-dispatch decision is computed at compile time in `node.rs`
// (`root_uses_arena_pure` and helpers) and cached on `CompiledLogic` as
// `uses_arena_dispatch`. This file just reads that bool in `evaluate()`.

/// Runtime peek for iterator-shaped arena roots: returns true iff the root
/// op's first arg resolves (via root borrow) to a `Value::Array` or `Null`.
/// Used to avoid arena setup when the input would force a value-mode bridge.
/// Conservatively returns true when we can't peek cheaply — the in-arena
/// `peek_root_value` will then bridge correctly if needed.
#[inline]
fn iter_root_input_is_array(root: &CompiledNode, data: &Value) -> bool {
    let CompiledNode::BuiltinOperator { args, .. } = root else {
        return true;
    };
    let Some(arg0) = args.first() else {
        return true;
    };
    let CompiledNode::CompiledVar {
        scope_level: 0,
        segments,
        reduce_hint: crate::node::ReduceHint::None,
        metadata_hint: crate::node::MetadataHint::None,
        default_value: None,
        ..
    } = arg0
    else {
        // Not a simple root var — could be a nested arena op, literal array, etc.
        // The full arena dispatch handles those correctly; let it run.
        return true;
    };
    let resolved = if segments.is_empty() {
        Some(data)
    } else {
        crate::operators::variable::try_traverse_segments(data, segments)
    };
    matches!(resolved, Some(Value::Array(_)) | Some(Value::Null) | None)
}

/// Peek at what an iterator's first arg would resolve to *without evaluating*.
/// Returns `Some(&Value)` only when the arg is a simple root-scope `var` — the
/// only case we can resolve with a borrow. For anything else (computed
/// collection, nested expression), returns `None` and the caller proceeds with
/// the regular arena dispatch (which will evaluate the arg properly). The
/// borrow lifetime is `'a` because `root` lives for the call's duration.
#[inline]
fn peek_root_value<'a>(
    arg: &CompiledNode,
    context: &ContextStack,
    root: &'a Value,
) -> Option<&'a Value> {
    if context.depth() != 0 {
        return None;
    }
    if let CompiledNode::CompiledVar {
        scope_level: 0,
        segments,
        reduce_hint: crate::node::ReduceHint::None,
        metadata_hint: crate::node::MetadataHint::None,
        default_value: None,
        ..
    } = arg
    {
        if segments.is_empty() {
            return Some(root);
        }
        return crate::operators::variable::try_traverse_segments(root, segments);
    }
    None
}

use crate::operators::variable;
#[cfg(feature = "trace")]
use crate::trace::{ExpressionNode, TraceCollector, TracedResult};
use crate::{
    CompiledLogic, CompiledNode, ContextStack, Error, Evaluator, Operator, Result, StructuredError,
};

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
    // No more builtin_operators array - OpCode handles dispatch directly!
    /// HashMap for custom operators only
    custom_operators: HashMap<String, Box<dyn Operator>>,
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
            custom_operators: HashMap::new(),
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
    /// use datalogic_rs::{DataLogic, Operator, ContextStack, Evaluator, Result, Error};
    /// use serde_json::{json, Value};
    /// use std::sync::Arc;
    ///
    /// struct UpperOperator;
    /// impl Operator for UpperOperator {
    ///     fn evaluate(&self, args: &[Value], context: &mut ContextStack,
    ///                 evaluator: &dyn Evaluator) -> Result<Value> {
    ///         let val = evaluator.evaluate(&args[0], context)?;
    ///         Ok(json!(val.as_str().unwrap_or("").to_uppercase()))
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
        Self {
            custom_operators: HashMap::new(),
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
            custom_operators: HashMap::new(),
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
            custom_operators: HashMap::new(),
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

    /// Registers a custom operator with the engine.
    ///
    /// Custom operators extend the engine's functionality with domain-specific logic.
    /// They override built-in operators if the same name is used.
    ///
    /// # Arguments
    ///
    /// * `name` - The operator name (e.g., "custom_calc")
    /// * `operator` - The operator implementation
    ///
    /// # Example
    ///
    /// ```rust
    /// use datalogic_rs::{DataLogic, Operator, ContextStack, Evaluator, Result, Error};
    /// use serde_json::{json, Value};
    ///
    /// struct DoubleOperator;
    ///
    /// impl Operator for DoubleOperator {
    ///     fn evaluate(
    ///         &self,
    ///         args: &[Value],
    ///         _context: &mut ContextStack,
    ///         _evaluator: &dyn Evaluator,
    ///     ) -> Result<Value> {
    ///         if let Some(n) = args.first().and_then(|v| v.as_f64()) {
    ///             Ok(json!(n * 2.0))
    ///         } else {
    ///             Err(Error::InvalidArguments("Argument must be a number".to_string()))
    ///         }
    ///     }
    /// }
    ///
    /// let mut engine = DataLogic::new();
    /// engine.add_operator("double".to_string(), Box::new(DoubleOperator));
    /// ```
    pub fn add_operator(&mut self, name: String, operator: Box<dyn Operator>) {
        self.custom_operators.insert(name, operator);
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
        // Hot path: cheap rules go straight to value-mode dispatch with no
        // intermediate branching. Arena routing is hoisted into a `#[cold]`
        // helper so its code lays out far from the hot fast path, freeing
        // L1i pressure for tiny rules (var / + / if/===) that dominate the
        // suite average.
        if compiled.uses_arena_dispatch {
            return self.evaluate_arena_dispatch(compiled, data);
        }
        let mut context = ContextStack::new(data);
        self.evaluate_node(&compiled.root, &mut context)
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
        if compiled.uses_arena_dispatch {
            return self.evaluate_arena_dispatch_ref(compiled, data);
        }
        // Value-mode bridge currently still needs `Arc<Value>` for `ContextStack`.
        // Clone the value into a fresh Arc — removed in a later stage when
        // `ContextStack` supports a borrowed root.
        let mut context = ContextStack::new(Arc::new(data.clone()));
        self.evaluate_node(&compiled.root, &mut context)
    }

    /// Cold arena dispatch trampoline. Decides between in-arena evaluation
    /// and a value-mode bridge (used when an iterator's input collection
    /// turns out to be a `Value::Object`, which `evaluate_via_arena` would
    /// just bridge back anyway — skipping arena setup is the win).
    #[cold]
    #[inline(never)]
    fn evaluate_arena_dispatch(
        &self,
        compiled: &CompiledLogic,
        data: Arc<Value>,
    ) -> Result<Value> {
        if compiled.arena_iter_root && !iter_root_input_is_array(&compiled.root, &data) {
            let mut context = ContextStack::new(data);
            return self.evaluate_node(&compiled.root, &mut context);
        }
        self.evaluate_via_arena(compiled, data)
    }

    /// Cold arena dispatch trampoline for the borrowed-data API.
    #[cold]
    #[inline(never)]
    fn evaluate_arena_dispatch_ref(
        &self,
        compiled: &CompiledLogic,
        data: &Value,
    ) -> Result<Value> {
        if compiled.arena_iter_root && !iter_root_input_is_array(&compiled.root, data) {
            // Value-mode bridge — clone for ContextStack until it supports borrowed root.
            let mut context = ContextStack::new(Arc::new(data.clone()));
            return self.evaluate_node(&compiled.root, &mut context);
        }
        self.evaluate_via_arena_ref(compiled, data)
    }

    /// Arena-mode evaluation entry. Acquires a thread-local `Bump` (from the
    /// pool, or freshly sized from the rule's compile-time hint), dispatches
    /// through `evaluate_arena_node`, and converts the result back to owned
    /// `Value` at the boundary. The arena is reset and returned to the pool
    /// when `guard` drops at end of function.
    #[cold]
    #[inline(never)]
    fn evaluate_via_arena(&self, compiled: &CompiledLogic, data: Arc<Value>) -> Result<Value> {
        use crate::arena::{ArenaGuard, arena_to_value};
        // Size hint for first-time pool fills: static_bytes × 2, min 4 KiB.
        let cap = compiled.arena_static_bytes.saturating_mul(2).max(4096);
        let guard = ArenaGuard::acquire(cap);
        let arena = guard.arena();
        // Clone the Arc (refcount-only, no Value clone) so we can borrow `&Value`
        // outside the ContextStack. Eliminates the unsafe lifetime cast that
        // previously bridged Arc-stored Value to `&'a Value`.
        let arc_for_borrow = Arc::clone(&data);
        let mut context = ContextStack::new(data);
        let root_ref: &Value = &arc_for_borrow;
        let result = self.evaluate_arena_node(&compiled.root, &mut context, arena, root_ref)?;
        let owned = arena_to_value(result);
        drop(guard);
        drop(arc_for_borrow);
        Ok(owned)
    }

    /// Borrowed-data variant of `evaluate_via_arena`. No Arc::clone — the
    /// caller's `&Value` lives on the caller's stack; we synthesize an Arc
    /// only for the legacy `ContextStack` bridge (removed in Stage E).
    #[cold]
    #[inline(never)]
    fn evaluate_via_arena_ref(&self, compiled: &CompiledLogic, data: &Value) -> Result<Value> {
        use crate::arena::{ArenaGuard, arena_to_value};
        let cap = compiled.arena_static_bytes.saturating_mul(2).max(4096);
        let guard = ArenaGuard::acquire(cap);
        let arena = guard.arena();
        // Synthetic Arc for the legacy ContextStack bridge. The clone goes
        // away when ContextStack supports borrowed-root or is removed from
        // the arena dispatch path.
        let mut context = ContextStack::new(Arc::new(data.clone()));
        let result = self.evaluate_arena_node(&compiled.root, &mut context, arena, data)?;
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
        let mut context = ContextStack::new(data);
        // Route through Structured mode so the breadcrumb push fires;
        // Plain callers never pay for it.
        let mut mode = crate::eval_mode::Structured;
        match self.evaluate_node_with_mode(&compiled.root, &mut context, &mut mode) {
            Ok(v) => Ok(v),
            Err(e) => {
                let path = context.take_error_path();
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

    /// Evaluates a compiled node using OpCode dispatch.
    ///
    /// This is the core evaluation method that handles:
    /// - Static values
    /// - Arrays
    /// - Built-in operators (via OpCode)
    /// - Custom operators
    /// - Structured objects (in preserve mode)
    ///
    /// # Arguments
    ///
    /// * `node` - The compiled node to evaluate
    /// * `context` - The context stack containing data and metadata
    ///
    /// # Returns
    ///
    /// The evaluation result, or an error if evaluation fails.
    /// Core generic dispatch. Parameterised over [`Mode`] so plain and traced
    /// execution share a single body. `Plain` monomorphisation DCEs the
    /// trace-only bookkeeping; `Traced` records a step per non-literal node.
    #[inline]
    pub fn evaluate_node_with_mode<M: crate::eval_mode::Mode>(
        &self,
        node: &CompiledNode,
        context: &mut ContextStack,
        mode: &mut M,
    ) -> Result<Value> {
        // Literals never emit trace steps (matches previous behaviour) and
        // don't need a context snapshot.
        if let CompiledNode::Value { value, .. } = node {
            return Ok(value.clone());
        }

        // Snapshot context data for tracing BEFORE children mutate it.
        // Under Plain the branch is const-false and gets DCE'd.
        let ctx_data = if M::TRACED {
            context.current().data().clone()
        } else {
            Value::Null
        };

        let result: Result<Value> = match node {
            CompiledNode::Value { .. } => unreachable!(),

            CompiledNode::Array { nodes, .. } => {
                let mut results = Vec::with_capacity(nodes.len());
                for n in nodes.iter() {
                    results.push(self.evaluate_node_with_mode::<M>(n, context, mode)?);
                }
                Ok(Value::Array(results))
            }

            CompiledNode::BuiltinOperator { opcode, args, .. } => {
                opcode.evaluate_with_mode::<M>(args, context, self, mode)
            }

            CompiledNode::CustomOperator(data) => {
                let operator = self
                    .custom_operators
                    .get(&data.name)
                    .ok_or_else(|| Error::InvalidOperator(data.name.clone()))?;

                let arg_values: Vec<Value> = data.args.iter().map(node_to_value).collect();
                let evaluator = SimpleEvaluator::new(self);

                operator.evaluate(&arg_values, context, &evaluator)
            }

            #[cfg(feature = "preserve")]
            CompiledNode::StructuredObject(data) => {
                let mut result = serde_json::Map::new();
                for (key, n) in data.fields.iter() {
                    let value = self.evaluate_node_with_mode::<M>(n, context, mode)?;
                    result.insert(key.clone(), value);
                }
                Ok(Value::Object(result))
            }

            CompiledNode::CompiledVar {
                scope_level,
                segments,
                reduce_hint,
                metadata_hint,
                default_value,
                ..
            } => variable::evaluate_compiled_var(
                *scope_level,
                segments,
                *reduce_hint,
                *metadata_hint,
                default_value.as_deref(),
                context,
                self,
            ),

            #[cfg(feature = "ext-control")]
            CompiledNode::CompiledExists(data) => {
                variable::evaluate_compiled_exists(data.scope_level, &data.segments, context)
            }

            #[cfg(feature = "ext-string")]
            CompiledNode::CompiledSplitRegex(data) => {
                use crate::operators::string;
                string::evaluate_split_with_regex(
                    &data.args,
                    context,
                    self,
                    &data.regex,
                    &data.capture_names,
                )
            }

            #[cfg(feature = "error-handling")]
            CompiledNode::CompiledThrow(data) => Err(Error::Thrown(data.error.clone())),
        };

        // Accumulate the error breadcrumb on every Err — but only when the
        // mode actually collects it. `M::TRACK_PATH` is `const bool`, so this
        // entire branch is DCE'd under `Plain` (zero cost on the hot path).
        // `Structured` and `Traced` callers pay one `is_err` check per node.
        if M::TRACK_PATH && result.is_err() {
            context.push_error_step(node.id());
        }

        if M::TRACED {
            mode.on_node_result(node, &ctx_data, &result);
        }
        result
    }

    /// Plain (untraced) dispatch. Thin wrapper around
    /// [`evaluate_node_with_mode`] specialised to [`Plain`](crate::eval_mode::Plain).
    #[inline]
    pub fn evaluate_node(&self, node: &CompiledNode, context: &mut ContextStack) -> Result<Value> {
        self.evaluate_node_with_mode(node, context, &mut crate::eval_mode::Plain)
    }

    /// Mode-aware Cow evaluator. Borrows literal values without cloning;
    /// full evaluates (with tracing threaded through `mode`) otherwise.
    #[inline]
    pub fn evaluate_node_cow_with_mode<'a, M: crate::eval_mode::Mode>(
        &self,
        node: &'a CompiledNode,
        context: &mut ContextStack,
        mode: &mut M,
    ) -> Result<Cow<'a, Value>> {
        match node {
            CompiledNode::Value { value, .. } => Ok(Cow::Borrowed(value)),
            _ => self
                .evaluate_node_with_mode::<M>(node, context, mode)
                .map(Cow::Owned),
        }
    }

    /// Plain (untraced) Cow wrapper. See [`evaluate_node_cow_with_mode`].
    #[inline]
    pub fn evaluate_node_cow<'a>(
        &self,
        node: &'a CompiledNode,
        context: &mut ContextStack,
    ) -> Result<Cow<'a, Value>> {
        self.evaluate_node_cow_with_mode(node, context, &mut crate::eval_mode::Plain)
    }

    /// Arena-mode dispatch (POC scope: filter / length / var-of-root).
    ///
    /// Returns `&'a ArenaValue<'a>`. For nodes not yet arena-migrated, falls
    /// back to the existing `evaluate_node` path and promotes the resulting
    /// `Value` into the arena via `value_to_arena`.
    ///
    /// `root` must outlive `'a` and is the data the caller passed to
    /// `evaluate()`. Used by `var` to return `InputRef`s without copying.
    #[inline]
    pub(crate) fn evaluate_arena_node<'a>(
        &self,
        node: &CompiledNode,
        context: &mut ContextStack,
        arena: &'a bumpalo::Bump,
        root: &'a Value,
    ) -> Result<&'a crate::arena::ArenaValue<'a>> {
        use crate::arena::{ArenaValue, value_to_arena};
        use crate::node::{MetadataHint, ReduceHint};

        // POC limitation: arena ops handle Array (and Null) inputs only. For
        // other collection shapes — Value::Object (object iteration semantics)
        // or scalar inputs (existing ops treat as 1-item) — bridge the entire
        // op to the value-mode dispatcher. Cheap to detect via root borrow
        // without re-evaluating args[0].
        if let CompiledNode::BuiltinOperator { opcode, args, .. } = node
            && matches!(
                opcode,
                crate::OpCode::Filter
                    | crate::OpCode::Map
                    | crate::OpCode::All
                    | crate::OpCode::Some
                    | crate::OpCode::None
                    | crate::OpCode::Reduce
            )
            && !args.is_empty()
            && let Some(v) = peek_root_value(&args[0], context, root)
            && !matches!(v, Value::Array(_) | Value::Null)
        {
            let result = self.evaluate_node(node, context)?;
            return Ok(arena.alloc(value_to_arena(&result, arena)));
        }

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
                *scope_level,
                segments,
                *reduce_hint,
                *metadata_hint,
                default_value.as_deref(),
                context,
                self,
                arena,
                root,
            ),

            // Compiled exists: full dispatch — root scope walks the input
            // directly, others bridge to the value-mode helper. Result is
            // always a Bool singleton.
            #[cfg(feature = "ext-control")]
            CompiledNode::CompiledExists(data) => {
                crate::operators::variable::evaluate_compiled_exists_arena(
                    data.scope_level,
                    &data.segments,
                    context,
                    root,
                )
            }

            // Value literal: arena-promote directly from the borrowed `&Value`.
            // Avoids the redundant `Value::clone()` that the fallback path
            // performs by going through `evaluate_node`.
            CompiledNode::Value { value, .. } => match value {
                Value::Null => Ok(crate::arena::pool::singleton_null()),
                Value::Bool(b) => Ok(crate::arena::pool::singleton_bool(*b)),
                Value::String(s) if s.is_empty() => {
                    Ok(crate::arena::pool::singleton_empty_string())
                }
                Value::Array(a) if a.is_empty() => {
                    Ok(crate::arena::pool::singleton_empty_array())
                }
                _ => Ok(arena.alloc(value_to_arena(value, arena))),
            },

            // Raw var/val/exists operator forms (rare — most are precompiled
            // to CompiledVar/CompiledExists, but dynamic-path forms remain
            // as BuiltinOperator).
            CompiledNode::BuiltinOperator {
                opcode: crate::OpCode::Var,
                args,
                ..
            } => crate::operators::variable::evaluate_var_arena(args, context, self, arena, root),
            #[cfg(feature = "ext-control")]
            CompiledNode::BuiltinOperator {
                opcode: crate::OpCode::Val,
                args,
                ..
            } => crate::operators::variable::evaluate_val_arena(args, context, self, arena, root),
            #[cfg(feature = "ext-control")]
            CompiledNode::BuiltinOperator {
                opcode: crate::OpCode::Exists,
                args,
                ..
            } => crate::operators::variable::evaluate_exists_arena(args, context, self, arena, root),

            CompiledNode::BuiltinOperator {
                opcode: crate::OpCode::Filter,
                args,
                ..
            } => crate::operators::array::evaluate_filter_arena(args, context, self, arena, root),

            CompiledNode::BuiltinOperator {
                opcode: crate::OpCode::Map,
                args,
                ..
            } => crate::operators::array::evaluate_map_arena(args, context, self, arena, root),

            CompiledNode::BuiltinOperator {
                opcode: crate::OpCode::All,
                args,
                ..
            } => crate::operators::array::evaluate_all_arena(args, context, self, arena, root),

            CompiledNode::BuiltinOperator {
                opcode: crate::OpCode::Some,
                args,
                ..
            } => crate::operators::array::evaluate_some_arena(args, context, self, arena, root),

            CompiledNode::BuiltinOperator {
                opcode: crate::OpCode::None,
                args,
                ..
            } => crate::operators::array::evaluate_none_arena(args, context, self, arena, root),

            CompiledNode::BuiltinOperator {
                opcode: crate::OpCode::Reduce,
                args,
                ..
            } => crate::operators::array::evaluate_reduce_arena(args, context, self, arena, root),

            CompiledNode::BuiltinOperator {
                opcode: crate::OpCode::Merge,
                args,
                ..
            } => crate::operators::array::evaluate_merge_arena(args, context, self, arena, root),

            CompiledNode::BuiltinOperator {
                opcode: crate::OpCode::Missing,
                args,
                ..
            } => crate::operators::missing::evaluate_missing_arena(args, context, self, arena, root),
            CompiledNode::BuiltinOperator {
                opcode: crate::OpCode::MissingSome,
                args,
                ..
            } => crate::operators::missing::evaluate_missing_some_arena(args, context, self, arena, root),

            #[cfg(feature = "ext-string")]
            CompiledNode::BuiltinOperator {
                opcode: crate::OpCode::Length,
                args,
                ..
            } => crate::operators::array::evaluate_length_arena(args, context, self, arena, root),

            #[cfg(feature = "ext-array")]
            CompiledNode::BuiltinOperator {
                opcode: crate::OpCode::Sort,
                args,
                ..
            } => crate::operators::array::evaluate_sort_arena(args, context, self, arena, root),

            CompiledNode::BuiltinOperator {
                opcode: crate::OpCode::Max,
                args,
                ..
            } => crate::operators::arithmetic::evaluate_max_arena(args, context, self, arena, root),
            CompiledNode::BuiltinOperator {
                opcode: crate::OpCode::Min,
                args,
                ..
            } => crate::operators::arithmetic::evaluate_min_arena(args, context, self, arena, root),
            CompiledNode::BuiltinOperator {
                opcode: crate::OpCode::Add,
                args,
                ..
            } => crate::operators::arithmetic::evaluate_add_arena(args, context, self, arena, root),
            CompiledNode::BuiltinOperator {
                opcode: crate::OpCode::Multiply,
                args,
                ..
            } => crate::operators::arithmetic::evaluate_multiply_arena(args, context, self, arena, root),

            // Comparison
            CompiledNode::BuiltinOperator {
                opcode: crate::OpCode::Equals,
                args,
                ..
            } => crate::operators::comparison::evaluate_equals_arena(args, context, self, arena, root),
            CompiledNode::BuiltinOperator {
                opcode: crate::OpCode::StrictEquals,
                args,
                ..
            } => crate::operators::comparison::evaluate_strict_equals_arena(args, context, self, arena, root),
            CompiledNode::BuiltinOperator {
                opcode: crate::OpCode::NotEquals,
                args,
                ..
            } => crate::operators::comparison::evaluate_not_equals_arena(args, context, self, arena, root),
            CompiledNode::BuiltinOperator {
                opcode: crate::OpCode::StrictNotEquals,
                args,
                ..
            } => crate::operators::comparison::evaluate_strict_not_equals_arena(args, context, self, arena, root),
            CompiledNode::BuiltinOperator {
                opcode: crate::OpCode::GreaterThan,
                args,
                ..
            } => crate::operators::comparison::evaluate_greater_than_arena(args, context, self, arena, root),
            CompiledNode::BuiltinOperator {
                opcode: crate::OpCode::GreaterThanEqual,
                args,
                ..
            } => crate::operators::comparison::evaluate_greater_than_equal_arena(args, context, self, arena, root),
            CompiledNode::BuiltinOperator {
                opcode: crate::OpCode::LessThan,
                args,
                ..
            } => crate::operators::comparison::evaluate_less_than_arena(args, context, self, arena, root),
            CompiledNode::BuiltinOperator {
                opcode: crate::OpCode::LessThanEqual,
                args,
                ..
            } => crate::operators::comparison::evaluate_less_than_equal_arena(args, context, self, arena, root),

            // Logical
            CompiledNode::BuiltinOperator {
                opcode: crate::OpCode::Not,
                args,
                ..
            } => crate::operators::logical::evaluate_not_arena(args, context, self, arena, root),
            CompiledNode::BuiltinOperator {
                opcode: crate::OpCode::DoubleNot,
                args,
                ..
            } => crate::operators::logical::evaluate_double_not_arena(args, context, self, arena, root),
            CompiledNode::BuiltinOperator {
                opcode: crate::OpCode::And,
                args,
                ..
            } => crate::operators::logical::evaluate_and_arena(args, context, self, arena, root),
            CompiledNode::BuiltinOperator {
                opcode: crate::OpCode::Or,
                args,
                ..
            } => crate::operators::logical::evaluate_or_arena(args, context, self, arena, root),

            // Control
            CompiledNode::BuiltinOperator {
                opcode: crate::OpCode::If,
                args,
                ..
            } => crate::operators::control::evaluate_if_arena(args, context, self, arena, root),
            CompiledNode::BuiltinOperator {
                opcode: crate::OpCode::Ternary,
                args,
                ..
            } => crate::operators::control::evaluate_ternary_arena(args, context, self, arena, root),
            #[cfg(feature = "ext-control")]
            CompiledNode::BuiltinOperator {
                opcode: crate::OpCode::Coalesce,
                args,
                ..
            } => crate::operators::control::evaluate_coalesce_arena(args, context, self, arena, root),
            #[cfg(feature = "ext-control")]
            CompiledNode::BuiltinOperator {
                opcode: crate::OpCode::Switch,
                args,
                ..
            } => crate::operators::control::evaluate_switch_arena(args, context, self, arena, root),

            // Arithmetic binary forms
            CompiledNode::BuiltinOperator {
                opcode: crate::OpCode::Subtract,
                args,
                ..
            } => crate::operators::arithmetic::evaluate_subtract_arena(args, context, self, arena, root),
            CompiledNode::BuiltinOperator {
                opcode: crate::OpCode::Divide,
                args,
                ..
            } => crate::operators::arithmetic::evaluate_divide_arena(args, context, self, arena, root),
            CompiledNode::BuiltinOperator {
                opcode: crate::OpCode::Modulo,
                args,
                ..
            } => crate::operators::arithmetic::evaluate_modulo_arena(args, context, self, arena, root),

            // Math (unary)
            #[cfg(feature = "ext-math")]
            CompiledNode::BuiltinOperator {
                opcode: crate::OpCode::Abs,
                args,
                ..
            } => crate::operators::arithmetic::evaluate_abs_arena(args, context, self, arena, root),
            #[cfg(feature = "ext-math")]
            CompiledNode::BuiltinOperator {
                opcode: crate::OpCode::Ceil,
                args,
                ..
            } => crate::operators::arithmetic::evaluate_ceil_arena(args, context, self, arena, root),
            #[cfg(feature = "ext-math")]
            CompiledNode::BuiltinOperator {
                opcode: crate::OpCode::Floor,
                args,
                ..
            } => crate::operators::arithmetic::evaluate_floor_arena(args, context, self, arena, root),

            // String
            CompiledNode::BuiltinOperator {
                opcode: crate::OpCode::Cat,
                args,
                ..
            } => crate::operators::string::evaluate_cat_arena(args, context, self, arena, root),
            CompiledNode::BuiltinOperator {
                opcode: crate::OpCode::Substr,
                args,
                ..
            } => crate::operators::string::evaluate_substr_arena(args, context, self, arena, root),
            CompiledNode::BuiltinOperator {
                opcode: crate::OpCode::In,
                args,
                ..
            } => crate::operators::string::evaluate_in_arena(args, context, self, arena, root),
            #[cfg(feature = "ext-string")]
            CompiledNode::BuiltinOperator {
                opcode: crate::OpCode::StartsWith,
                args,
                ..
            } => crate::operators::string::evaluate_starts_with_arena(args, context, self, arena, root),
            #[cfg(feature = "ext-string")]
            CompiledNode::BuiltinOperator {
                opcode: crate::OpCode::EndsWith,
                args,
                ..
            } => crate::operators::string::evaluate_ends_with_arena(args, context, self, arena, root),
            #[cfg(feature = "ext-string")]
            CompiledNode::BuiltinOperator {
                opcode: crate::OpCode::Upper,
                args,
                ..
            } => crate::operators::string::evaluate_upper_arena(args, context, self, arena, root),
            #[cfg(feature = "ext-string")]
            CompiledNode::BuiltinOperator {
                opcode: crate::OpCode::Lower,
                args,
                ..
            } => crate::operators::string::evaluate_lower_arena(args, context, self, arena, root),
            #[cfg(feature = "ext-string")]
            CompiledNode::BuiltinOperator {
                opcode: crate::OpCode::Trim,
                args,
                ..
            } => crate::operators::string::evaluate_trim_arena(args, context, self, arena, root),
            #[cfg(feature = "ext-string")]
            CompiledNode::BuiltinOperator {
                opcode: crate::OpCode::Split,
                args,
                ..
            } => crate::operators::string::evaluate_split_arena(args, context, self, arena, root),

            // DateTime
            #[cfg(feature = "datetime")]
            CompiledNode::BuiltinOperator {
                opcode: crate::OpCode::Datetime,
                args,
                ..
            } => crate::operators::datetime::evaluate_datetime_arena(args, context, self, arena, root),
            #[cfg(feature = "datetime")]
            CompiledNode::BuiltinOperator {
                opcode: crate::OpCode::Timestamp,
                args,
                ..
            } => crate::operators::datetime::evaluate_timestamp_arena(args, context, self, arena, root),
            #[cfg(feature = "datetime")]
            CompiledNode::BuiltinOperator {
                opcode: crate::OpCode::ParseDate,
                args,
                ..
            } => crate::operators::datetime::evaluate_parse_date_arena(args, context, self, arena, root),
            #[cfg(feature = "datetime")]
            CompiledNode::BuiltinOperator {
                opcode: crate::OpCode::FormatDate,
                args,
                ..
            } => crate::operators::datetime::evaluate_format_date_arena(args, context, self, arena, root),
            #[cfg(feature = "datetime")]
            CompiledNode::BuiltinOperator {
                opcode: crate::OpCode::DateDiff,
                args,
                ..
            } => crate::operators::datetime::evaluate_date_diff_arena(args, context, self, arena, root),
            #[cfg(feature = "datetime")]
            CompiledNode::BuiltinOperator {
                opcode: crate::OpCode::Now,
                args,
                ..
            } => crate::operators::datetime::evaluate_now_arena(args, context, self, arena, root),

            // Type
            #[cfg(feature = "ext-control")]
            CompiledNode::BuiltinOperator {
                opcode: crate::OpCode::Type,
                args,
                ..
            } => crate::operators::type_op::evaluate_type_arena(args, context, self, arena, root),

            // Throw / Try
            #[cfg(feature = "error-handling")]
            CompiledNode::BuiltinOperator {
                opcode: crate::OpCode::Throw,
                args,
                ..
            } => crate::operators::throw::evaluate_throw_arena(args, context, self, arena, root),
            #[cfg(feature = "error-handling")]
            CompiledNode::BuiltinOperator {
                opcode: crate::OpCode::Try,
                args,
                ..
            } => crate::operators::try_op::evaluate_try_arena(args, context, self, arena, root),

            // Preserve
            #[cfg(feature = "preserve")]
            CompiledNode::BuiltinOperator {
                opcode: crate::OpCode::Preserve,
                args,
                ..
            } => crate::operators::preserve::evaluate_preserve_arena(args, context, self, arena, root),

            // Slice
            #[cfg(feature = "ext-array")]
            CompiledNode::BuiltinOperator {
                opcode: crate::OpCode::Slice,
                args,
                ..
            } => crate::operators::array::evaluate_slice_arena(args, context, self, arena, root),

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
                    let val_av = self.evaluate_arena_node(n, context, arena, root)?;
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
                    let av = self.evaluate_arena_node(n, context, arena, root)?;
                    items.push(crate::arena::value::reborrow_arena_value(av));
                }
                Ok(arena.alloc(ArenaValue::Array(items.into_bump_slice())))
            }

            // Custom operator: pre-evaluate each arg via arena dispatch (so
            // var lookups borrow), convert each to owned Value, call the
            // user's operator, wrap the result back into the arena. The
            // round-trip cost is bounded by the user op's arg count.
            CompiledNode::CustomOperator(data) => {
                let operator = self
                    .custom_operators
                    .get(&data.name)
                    .ok_or_else(|| Error::InvalidOperator(data.name.clone()))?;
                let mut owned_args: Vec<Value> = Vec::with_capacity(data.args.len());
                for arg in data.args.iter() {
                    let av = self.evaluate_arena_node(arg, context, arena, root)?;
                    owned_args.push(crate::arena::arena_to_value(av));
                }
                let evaluator = SimpleEvaluator::new(self);
                // Translate evaluated args into synthetic CompiledNode::Value
                // entries so the user's op sees pre-resolved values via
                // evaluator.evaluate. Existing custom-op contract preserved.
                let synth_args: Vec<Value> = owned_args;
                let result = operator.evaluate(&synth_args, context, &evaluator)?;
                Ok(arena.alloc(value_to_arena(&result, arena)))
            }

            // CompiledSplitRegex (ext-string regex split): build the result
            // object directly in the arena.
            #[cfg(feature = "ext-string")]
            CompiledNode::CompiledSplitRegex(data) => {
                crate::operators::string::evaluate_split_with_regex_arena(
                    &data.args,
                    context,
                    self,
                    &data.regex,
                    &data.capture_names,
                    arena,
                    root,
                )
            }

            // Fallback: bridge through the existing value-mode evaluator and
            // promote the result into the arena. No win at this point but
            // composition still works (parent arena ops can consume us).
            _ => {
                let v = self.evaluate_node(node, context)?;
                Ok(arena.alloc(value_to_arena(&v, arena)))
            }
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

        // Build expression tree and node ID mapping
        let expression_tree = ExpressionNode::build_from_compiled(&compiled.root);

        // Create context and trace collector
        let mut context = ContextStack::new(data_arc);
        let mut collector = TraceCollector::new();

        // Evaluate with tracing
        let result = self.evaluate_node_traced(&compiled.root, &mut context, &mut collector);

        match result {
            Ok(value) => Ok(TracedResult {
                result: value,
                expression_tree,
                steps: collector.into_steps(),
                error: None,
                error_structured: None,
            }),
            Err(e) => {
                // Return error but include partial steps for debugging
                let message = e.to_string();
                let path = context.take_error_path();
                let mut structured = StructuredError::from(e).with_path(path);
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
        let mut context = ContextStack::new(data_arc);
        let mut collector = TraceCollector::new();

        let result = self.evaluate_node_traced(&compiled.root, &mut context, &mut collector);

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
                let path = context.take_error_path();
                let mut structured = StructuredError::from(e).with_path(path);
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

    /// Traced dispatch. Thin wrapper around [`evaluate_node_with_mode`]
    /// specialised to [`Traced`](crate::eval_mode::Traced). Node IDs come
    /// directly from each [`CompiledNode::id`] — no pointer-keyed side-table.
    #[cfg(feature = "trace")]
    pub fn evaluate_node_traced(
        &self,
        node: &CompiledNode,
        context: &mut ContextStack,
        collector: &mut TraceCollector,
    ) -> Result<Value> {
        let mut traced = crate::eval_mode::Traced { collector };
        self.evaluate_node_with_mode(node, context, &mut traced)
    }
}

// node_to_value, segments_to_dot_path, and segment_to_value are in node.rs
use crate::node::node_to_value;

/// Simple evaluator that compiles and evaluates without caching
struct SimpleEvaluator<'e> {
    engine: &'e DataLogic,
}

impl<'e> SimpleEvaluator<'e> {
    /// Create a new SimpleEvaluator
    fn new(engine: &'e DataLogic) -> Self {
        Self { engine }
    }
}

impl Evaluator for SimpleEvaluator<'_> {
    fn evaluate(&self, logic: &Value, context: &mut ContextStack) -> Result<Value> {
        // Compile and evaluate - compilation already handles simple values efficiently
        match logic {
            Value::Object(obj) if obj.len() == 1 => {
                let compiled = CompiledLogic::compile_with_static_eval(logic, self.engine)?;
                self.engine.evaluate_node(&compiled.root, context)
            }
            #[cfg(feature = "preserve")]
            Value::Object(obj) if obj.len() > 1 && self.engine.preserve_structure => {
                // Multi-key object in preserve_structure mode
                let compiled = CompiledLogic::compile_with_static_eval(logic, self.engine)?;
                self.engine.evaluate_node(&compiled.root, context)
            }
            Value::Array(_) => {
                let compiled = CompiledLogic::compile_with_static_eval(logic, self.engine)?;
                self.engine.evaluate_node(&compiled.root, context)
            }
            _ => Ok(logic.clone()),
        }
    }
}
