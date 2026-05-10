//! Execution tracing for step-by-step debugging.
//!
//! This module provides execution tracing capabilities for debugging JSONLogic
//! expressions. It generates an expression tree with unique IDs and records
//! each evaluation step for replay in the Web UI.
//!
//! # Feature gating
//!
//! Gated on `feature = "trace"`. Trace transitively pulls in
//! `feature = "serde_json"` (the `Cargo.toml` declares
//! `trace = ["serde_json"]`) because the per-step expression tree and
//! recorded values are `serde_json::Value`-shaped — the structured-trace
//! consumers (the Web UI, JSON exporters) need the JSON↔arena bridge to
//! render steps. `--features trace` implicitly enables `serde_json`.

use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::node_serialize;
use crate::{CompiledNode, Error};

/// Represents a node in the expression tree for flow diagram rendering.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExpressionNode {
    /// Unique identifier for this node
    pub id: u32,
    /// JSON string of this sub-expression
    pub expression: String,
    /// Child nodes (arguments/operands that are operators, not literals)
    pub children: Vec<ExpressionNode>,
}

impl ExpressionNode {
    /// Build an expression tree from a CompiledNode.
    ///
    /// Every tree node inherits its compile-time id from the source
    /// [`CompiledNode::id`]. No side-table is needed: both tracing and error
    /// reporting look the id up directly on the node.
    pub(crate) fn build_from_compiled(node: &CompiledNode) -> ExpressionNode {
        Self::build_node(node)
    }

    fn build_node(node: &CompiledNode) -> ExpressionNode {
        let id = node.id();
        match node {
            CompiledNode::Value { value, .. } => Self::leaf(id, value.to_json_string()),
            CompiledNode::Array { nodes, .. } => ExpressionNode {
                id,
                expression: node_serialize::node_to_json_string(node),
                children: Self::op_children(nodes),
            },
            CompiledNode::BuiltinOperator { opcode, args, .. } => ExpressionNode {
                id,
                expression: node_serialize::builtin_to_json_string(opcode, args),
                children: Self::op_children(args),
            },
            CompiledNode::CustomOperator(data) => ExpressionNode {
                id,
                expression: node_serialize::custom_to_json_string(&data.name, &data.args),
                children: Self::op_children(&data.args),
            },
            #[cfg(feature = "templating")]
            CompiledNode::StructuredObject(data) => ExpressionNode {
                id,
                expression: node_serialize::structured_to_json_string(&data.fields),
                children: Self::op_children_from_fields(&data.fields),
            },
            CompiledNode::Var {
                scope_level,
                segments,
                default_value,
                ..
            } => Self::build_compiled_var(id, *scope_level, segments, default_value.as_deref()),
            #[cfg(feature = "ext-control")]
            CompiledNode::Exists(data) => Self::leaf(
                id,
                node_serialize::compiled_exists_to_json_string(&data.segments),
            ),
            #[cfg(feature = "error-handling")]
            CompiledNode::Throw(_) | CompiledNode::Missing(_) | CompiledNode::MissingSome(_) => {
                Self::leaf(id, node_serialize::node_to_json_string(node))
            }
            #[cfg(not(feature = "error-handling"))]
            CompiledNode::Missing(_) | CompiledNode::MissingSome(_) => {
                Self::leaf(id, node_serialize::node_to_json_string(node))
            }
            CompiledNode::InvalidArgs { .. } => {
                Self::leaf(id, "{\"<invalid args>\": null}".to_string())
            }
        }
    }

    /// Build a leaf `ExpressionNode` (no children).
    #[inline]
    fn leaf(id: u32, expression: String) -> ExpressionNode {
        ExpressionNode {
            id,
            expression,
            children: vec![],
        }
    }

    /// Recurse into a compiled-node slice, keeping only the operator nodes
    /// (literals don't appear as flow-diagram children).
    #[inline]
    fn op_children(nodes: &[CompiledNode]) -> Vec<ExpressionNode> {
        nodes
            .iter()
            .filter(|n| Self::is_operator_node(n))
            .map(Self::build_node)
            .collect()
    }

    /// `op_children` for the `(name, CompiledNode)` shape used by structured
    /// object fields.
    #[cfg(feature = "templating")]
    #[inline]
    fn op_children_from_fields(fields: &[(String, CompiledNode)]) -> Vec<ExpressionNode> {
        fields
            .iter()
            .filter(|(_, n)| Self::is_operator_node(n))
            .map(|(_, n)| Self::build_node(n))
            .collect()
    }

    /// `CompiledVar`'s expression node — the only operator-shaped variant
    /// whose "child" is the optional default value rather than a fixed args
    /// slice.
    fn build_compiled_var(
        id: u32,
        scope_level: u32,
        segments: &[crate::node::PathSegment],
        default_value: Option<&CompiledNode>,
    ) -> ExpressionNode {
        let mut children = Vec::new();
        if let Some(def) = default_value {
            if Self::is_operator_node(def) {
                children.push(Self::build_node(def));
            }
        }
        ExpressionNode {
            id,
            expression: node_serialize::compiled_var_to_json_string(
                scope_level,
                segments,
                default_value,
            ),
            children,
        }
    }

    /// Check if a node is an operator (not a literal value)
    fn is_operator_node(node: &CompiledNode) -> bool {
        !matches!(node, CompiledNode::Value { .. })
    }
}

/// Captures state at each evaluation step.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExecutionStep {
    /// Sequential step number (assigned by the trace collector in
    /// recording order). Distinct from `node_id`, which is the
    /// compiled-node id of the expression being evaluated — `step_id`
    /// is the *order* this step occurred, `node_id` is *which node* ran.
    pub step_id: u32,
    /// ID of the node being evaluated
    pub node_id: u32,
    /// Current context/scope data at this step
    pub context: Value,
    /// Result after evaluating this node (None if error)
    pub result: Option<Value>,
    /// Error message if evaluation failed (None if success)
    pub error: Option<String>,
    /// Current iteration index (only for iterator body evaluations)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub iteration_index: Option<u32>,
    /// Total iteration count (only for iterator body evaluations)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub iteration_total: Option<u32>,
}

/// Collector for execution steps during traced evaluation.
pub(crate) struct TraceCollector {
    /// Recorded execution steps
    steps: Vec<ExecutionStep>,
    /// Counter for generating step IDs
    step_counter: u32,
    /// Stack of iteration info (index, total) for nested iterations
    iteration_stack: Vec<(u32, u32)>,
}

impl TraceCollector {
    /// Create a new trace collector
    pub fn new() -> Self {
        Self {
            steps: Vec::new(),
            step_counter: 0,
            iteration_stack: Vec::new(),
        }
    }

    /// Record a successful execution step
    pub fn record_step(&mut self, node_id: u32, context: Value, result: Value) {
        let (iteration_index, iteration_total) = self.current_iteration();
        let step = ExecutionStep {
            step_id: self.step_counter,
            node_id,
            context,
            result: Some(result),
            error: None,
            iteration_index,
            iteration_total,
        };
        self.steps.push(step);
        self.step_counter += 1;
    }

    /// Record an error execution step
    pub fn record_error(&mut self, node_id: u32, context: Value, error: String) {
        let (iteration_index, iteration_total) = self.current_iteration();
        let step = ExecutionStep {
            step_id: self.step_counter,
            node_id,
            context,
            result: None,
            error: Some(error),
            iteration_index,
            iteration_total,
        };
        self.steps.push(step);
        self.step_counter += 1;
    }

    /// Push iteration context for map/filter/reduce operations
    pub fn push_iteration(&mut self, index: u32, total: u32) {
        self.iteration_stack.push((index, total));
    }

    /// Pop iteration context
    pub fn pop_iteration(&mut self) {
        self.iteration_stack.pop();
    }

    /// Get current iteration info if inside an iteration
    fn current_iteration(&self) -> (Option<u32>, Option<u32>) {
        self.iteration_stack
            .last()
            .map(|(i, t)| (Some(*i), Some(*t)))
            .unwrap_or((None, None))
    }

    /// Consume the collector and return the recorded steps
    pub fn into_steps(self) -> Vec<ExecutionStep> {
        self.steps
    }
}

impl Default for TraceCollector {
    fn default() -> Self {
        Self::new()
    }
}

// ============================================================================
// v5 trace surface — `engine.trace().evaluate*(...)` returning `TracedRun`.
// ============================================================================

/// Result of a traced evaluation produced by [`TracedSession`]. Always
/// includes the trace data; the value-or-error split lives on
/// [`Self::result`].
#[derive(Debug, Clone)]
pub struct TracedRun<R> {
    /// `Ok(value)` on success, `Err(error)` on failure. The error always
    /// carries the operator + path metadata populated by the engine.
    pub result: Result<R, Error>,
    /// Per-node execution log captured during the run.
    pub steps: Vec<ExecutionStep>,
    /// Compile-time expression tree for flow-diagram rendering.
    pub expression_tree: ExpressionNode,
}

/// Trace-enabled view over a [`crate::Engine`] engine. Constructed via
/// [`crate::Engine::trace`]. Mirrors [`crate::Session`] 1:1 — every
/// `eval*` returns a [`TracedRun<R>`] carrying the trace alongside the
/// result, where `R` is the same shape that `Session::eval*` would
/// return. Owns its own [`bumpalo::Bump`] across calls; reset is
/// per-call (the trace path always allocates a fresh arena to keep the
/// borrowed-result lifetime tied to the run).
pub struct TracedSession<'e> {
    engine: &'e crate::Engine,
}

impl<'e> TracedSession<'e> {
    /// Construct a session over `engine`. Invoked from
    /// [`crate::Engine::trace`].
    #[inline]
    pub(crate) fn new(engine: &'e crate::Engine) -> Self {
        Self { engine }
    }

    /// Traced evaluation of a pre-compiled [`crate::Logic`] returning
    /// [`datavalue::OwnedDataValue`]. The trace surfaces only the
    /// operators that survived compilation — constant sub-expressions
    /// folded by [`crate::Engine::compile`] won't appear as steps. For
    /// full coverage on a one-shot run, prefer [`Self::eval_str`].
    pub fn eval<D>(&self, compiled: &crate::Logic, data: D) -> TracedRun<datavalue::OwnedDataValue>
    where
        D: crate::OwnedInput,
    {
        let owned_data = match data.into_owned_input() {
            Ok(d) => d,
            Err(e) => return Self::compile_failed(e),
        };
        let arena = bumpalo::Bump::new();
        let inner = self.eval_borrowed_in(compiled, &owned_data, &arena);
        TracedRun {
            result: inner.result.and_then(crate::FromDataValue::from_arena),
            steps: inner.steps,
            expression_tree: inner.expression_tree,
        }
    }

    /// One-shot traced evaluation with JSON-string boundary on both
    /// sides. Compiles internally with the optimizer + constant-fold
    /// passes disabled, so the trace surfaces every operator in the
    /// rule.
    pub fn eval_str<R, D>(&self, rule: R, data: D) -> TracedRun<String>
    where
        R: crate::IntoLogic,
        D: crate::OwnedInput,
    {
        let owned = match rule.into_owned_logic() {
            Ok(o) => o,
            Err(e) => return Self::compile_failed(e),
        };
        let compiled = match crate::Logic::compile_for_trace(&owned, self.engine) {
            Ok(c) => c,
            Err(e) => return Self::compile_failed(e),
        };
        let owned_data = match data.into_owned_input() {
            Ok(d) => d,
            Err(e) => return Self::compile_failed(e),
        };
        let arena = bumpalo::Bump::new();
        let inner = self.eval_borrowed_in(&compiled, &owned_data, &arena);
        TracedRun {
            result: inner.result.map(|v| v.to_string()),
            steps: inner.steps,
            expression_tree: inner.expression_tree,
        }
    }

    /// Typed traced evaluation: deserialise the result into
    /// `T: DeserializeOwned`. Routes through `serde_json`.
    #[cfg(feature = "serde_json")]
    #[cfg_attr(docsrs, doc(cfg(feature = "serde_json")))]
    pub fn eval_into<T, R, D>(&self, rule: R, data: D) -> TracedRun<T>
    where
        T: serde::de::DeserializeOwned,
        R: crate::IntoLogic,
        D: crate::OwnedInput,
    {
        let owned = match rule.into_owned_logic() {
            Ok(o) => o,
            Err(e) => return Self::compile_failed(e),
        };
        let compiled = match crate::Logic::compile_for_trace(&owned, self.engine) {
            Ok(c) => c,
            Err(e) => return Self::compile_failed(e),
        };
        let owned_data = match data.into_owned_input() {
            Ok(d) => d,
            Err(e) => return Self::compile_failed(e),
        };
        let arena = bumpalo::Bump::new();
        let inner = self.eval_borrowed_in(&compiled, &owned_data, &arena);
        let result = inner.result.and_then(|v| {
            let value: serde_json::Value = crate::FromDataValue::from_arena(v)?;
            serde_json::from_value(value).map_err(crate::Error::from)
        });
        TracedRun {
            result,
            steps: inner.steps,
            expression_tree: inner.expression_tree,
        }
    }

    /// Traced borrowed evaluation against a caller-owned arena. Mirrors
    /// [`crate::Session::eval_borrowed`] / [`crate::Engine::evaluate`]
    /// — the result references `arena`, while the trace data is owned
    /// and outlives the arena.
    pub fn eval_borrowed<'a, D>(
        &self,
        compiled: &'a crate::Logic,
        data: D,
        arena: &'a bumpalo::Bump,
    ) -> TracedRun<&'a crate::DataValue<'a>>
    where
        D: crate::EvalInput<'a>,
    {
        self.eval_borrowed_in(compiled, data, arena)
    }

    /// Internal: shared body for the borrowed-result trace runs.
    fn eval_borrowed_in<'a, D>(
        &self,
        compiled: &'a crate::Logic,
        data: D,
        arena: &'a bumpalo::Bump,
    ) -> TracedRun<&'a crate::DataValue<'a>>
    where
        D: crate::EvalInput<'a>,
    {
        let expression_tree = ExpressionNode::build_from_compiled(&compiled.root);
        let _depth_guard = match self.engine.enter_dispatch_boundary() {
            Ok(g) => g,
            Err(e) => {
                return TracedRun {
                    expression_tree,
                    steps: TraceCollector::new().into_steps(),
                    result: Err(e),
                };
            }
        };
        let data_ref = match data.into_arena_value(arena) {
            Ok(av) => av,
            Err(e) => {
                return TracedRun {
                    expression_tree,
                    steps: TraceCollector::new().into_steps(),
                    result: Err(e),
                };
            }
        };
        let mut ctx = crate::arena::ContextStack::new(data_ref);
        ctx.attach_tracer(TraceCollector::new());

        let outcome = self.engine.dispatch_node(&compiled.root, &mut ctx, arena);
        let result = match outcome {
            Ok(av) => Ok(av),
            Err(e) => Err(e.decorated(ctx.take_error_path(), compiled, false)),
        };
        let collector = ctx.detach_tracer().expect("attach_tracer was called above");
        TracedRun {
            result,
            steps: collector.into_steps(),
            expression_tree,
        }
    }

    fn compile_failed<R>(error: crate::Error) -> TracedRun<R> {
        TracedRun {
            result: Err(error),
            steps: Vec::new(),
            expression_tree: ExpressionNode {
                id: 0,
                expression: String::new(),
                children: Vec::new(),
            },
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::OpCode;

    #[test]
    fn test_expression_node_from_simple_operator() {
        // Create a simple {"val": "age"} node (var is normalized to Val).
        let node = CompiledNode::BuiltinOperator {
            id: crate::node::SYNTHETIC_ID,
            opcode: OpCode::Val,
            args: vec![CompiledNode::synthetic_value(
                datavalue::OwnedDataValue::from("age"),
            )]
            .into_boxed_slice(),
            predicate_hint: None,
            iter_arg_kind: crate::operators::array::IterArgKind::General,
        };

        let tree = ExpressionNode::build_from_compiled(&node);

        // Synthetic test nodes all share SYNTHETIC_ID, which surfaces as 0
        // through the public `ExpressionNode::id` (u32) shape; the
        // structural assertions below still hold.
        assert_eq!(tree.id, 0);
        assert_eq!(tree.expression, r#"{"val": "age"}"#);
        assert!(tree.children.is_empty()); // "age" is a literal, not a child
    }

    #[test]
    fn test_expression_node_from_nested_operator() {
        // Create {">=": [{"val": "age"}, 18]}
        let var_node = CompiledNode::BuiltinOperator {
            id: crate::node::SYNTHETIC_ID,
            opcode: OpCode::Val,
            args: vec![CompiledNode::synthetic_value(
                datavalue::OwnedDataValue::from("age"),
            )]
            .into_boxed_slice(),
            predicate_hint: None,
            iter_arg_kind: crate::operators::array::IterArgKind::General,
        };
        let node = CompiledNode::BuiltinOperator {
            id: crate::node::SYNTHETIC_ID,
            opcode: OpCode::GreaterThanEqual,
            args: vec![
                var_node,
                CompiledNode::synthetic_value(datavalue::OwnedDataValue::Number(
                    datavalue::NumberValue::Integer(18),
                )),
            ]
            .into_boxed_slice(),
            predicate_hint: None,
            iter_arg_kind: crate::operators::array::IterArgKind::General,
        };

        let tree = ExpressionNode::build_from_compiled(&node);

        assert_eq!(tree.id, 0);
        assert!(tree.expression.contains(">="));
        assert_eq!(tree.children.len(), 1); // var node is a child
        assert!(tree.children[0].expression.contains("val"));
    }

    #[test]
    fn test_trace_collector_records_steps() {
        let mut collector = TraceCollector::new();

        collector.record_step(0, serde_json::json!({"age": 25}), serde_json::json!(25));
        collector.record_step(1, serde_json::json!({"age": 25}), serde_json::json!(true));

        let steps = collector.into_steps();
        assert_eq!(steps.len(), 2);
        assert_eq!(steps[0].step_id, 0);
        assert_eq!(steps[0].node_id, 0);
        assert_eq!(steps[1].step_id, 1);
        assert_eq!(steps[1].node_id, 1);
    }

    #[test]
    fn test_trace_collector_iteration_context() {
        let mut collector = TraceCollector::new();

        collector.push_iteration(0, 3);
        collector.record_step(2, serde_json::json!(1), serde_json::json!(2));

        let steps = collector.into_steps();
        assert_eq!(steps[0].iteration_index, Some(0));
        assert_eq!(steps[0].iteration_total, Some(3));
    }

    #[test]
    fn traced_session_evaluate_str_smoke() {
        let engine = crate::Engine::new();
        let run = engine.trace().eval_str(r#"{"+": [1, 2, 3]}"#, "null");
        assert_eq!(run.result.unwrap(), "6");
        // The one-shot trace path skips static folding internally, so the
        // `+` operator survives and produces a step.
        assert!(!run.steps.is_empty(), "expected non-empty steps");
        assert_ne!(run.expression_tree.id, 0);
    }

    #[test]
    fn traced_pre_compiled_inherits_fold() {
        // Pre-compiled trace inherits the shape from `Engine::compile`, which
        // folds. A fully-constant rule has no surviving operator → no steps.
        let engine = crate::Engine::new();
        let compiled = engine.compile(r#"{"+": [1, 2]}"#).unwrap();
        let arena = bumpalo::Bump::new();
        let data = datavalue::DataValue::from_str("null", &arena).unwrap();
        let run = engine.trace().eval_borrowed(&compiled, data, &arena);
        assert_eq!(run.result.as_ref().unwrap().as_i64(), Some(3));
        assert!(
            run.steps.is_empty(),
            "folded rule should not produce trace steps"
        );
    }

    #[test]
    fn traced_session_carries_error_metadata() {
        let engine = crate::Engine::new();
        let run = engine.trace().eval_str(r#"{"+": ["x", 1]}"#, "null");
        let err = run.result.expect_err("string-arith should fail");
        assert_eq!(err.operator(), Some("+"));
        assert!(!err.node_ids().is_empty(), "expected populated breadcrumb");
    }
}
