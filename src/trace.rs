//! Execution tracing for step-by-step debugging.
//!
//! This module provides execution tracing capabilities for debugging JSONLogic
//! expressions. It generates an expression tree with unique IDs and records
//! each evaluation step for replay in the Web UI.

use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::{CompiledNode, OpCode, StructuredError};

/// The result of a traced evaluation, containing both the result and execution trace.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TracedResult {
    /// The evaluation result
    pub result: Value,
    /// Expression tree with unique IDs for flow diagram rendering
    pub expression_tree: ExpressionNode,
    /// Ordered execution steps for replay
    pub steps: Vec<ExecutionStep>,
    /// Top-level error message if evaluation failed
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
    /// Structured top-level error if evaluation failed. Serialize-only —
    /// ignored on deserialize (StructuredError is a Rust→JS shape).
    #[serde(skip_serializing_if = "Option::is_none", skip_deserializing, default)]
    pub error_structured: Option<StructuredError>,
}

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
    pub fn build_from_compiled(node: &CompiledNode) -> ExpressionNode {
        Self::build_node(node)
    }

    fn build_node(node: &CompiledNode) -> ExpressionNode {
        let id = node.id();

        match node {
            CompiledNode::Value { value, .. } => ExpressionNode {
                id,
                expression: value.to_string(),
                children: vec![],
            },
            CompiledNode::Array { nodes, .. } => {
                let children: Vec<ExpressionNode> = nodes
                    .iter()
                    .filter(|n| Self::is_operator_node(n))
                    .map(Self::build_node)
                    .collect();
                ExpressionNode {
                    id,
                    expression: Self::node_to_json_string(node),
                    children,
                }
            }
            CompiledNode::BuiltinOperator { opcode, args, .. } => {
                let children: Vec<ExpressionNode> = args
                    .iter()
                    .filter(|n| Self::is_operator_node(n))
                    .map(Self::build_node)
                    .collect();
                ExpressionNode {
                    id,
                    expression: Self::builtin_to_json_string(opcode, args),
                    children,
                }
            }
            CompiledNode::CustomOperator(data) => {
                let children: Vec<ExpressionNode> = data
                    .args
                    .iter()
                    .filter(|n| Self::is_operator_node(n))
                    .map(Self::build_node)
                    .collect();
                ExpressionNode {
                    id,
                    expression: Self::custom_to_json_string(&data.name, &data.args),
                    children,
                }
            }
            #[cfg(feature = "preserve")]
            CompiledNode::StructuredObject(data) => {
                let children: Vec<ExpressionNode> = data
                    .fields
                    .iter()
                    .filter(|(_, n)| Self::is_operator_node(n))
                    .map(|(_, n)| Self::build_node(n))
                    .collect();
                ExpressionNode {
                    id,
                    expression: Self::structured_to_json_string(&data.fields),
                    children,
                }
            }

            CompiledNode::CompiledVar {
                scope_level,
                segments,
                default_value,
                ..
            } => {
                let mut children = Vec::new();
                if let Some(def) = default_value
                    && Self::is_operator_node(def)
                {
                    children.push(Self::build_node(def));
                }
                ExpressionNode {
                    id,
                    expression: Self::compiled_var_to_json_string(
                        *scope_level,
                        segments,
                        default_value.as_deref(),
                    ),
                    children,
                }
            }

            #[cfg(feature = "ext-control")]
            CompiledNode::CompiledExists(data) => ExpressionNode {
                id,
                expression: Self::compiled_exists_to_json_string(data.scope_level, &data.segments),
                children: vec![],
            },

            #[cfg(feature = "ext-string")]
            CompiledNode::CompiledSplitRegex(data) => {
                let children: Vec<ExpressionNode> = data
                    .args
                    .iter()
                    .filter(|n| Self::is_operator_node(n))
                    .map(Self::build_node)
                    .collect();
                ExpressionNode {
                    id,
                    expression: format!(
                        "{{\"split\": [{}, \"{}\"]}}",
                        Self::node_to_json_string(&data.args[0]),
                        data.regex.as_str()
                    ),
                    children,
                }
            }

            #[cfg(feature = "error-handling")]
            CompiledNode::CompiledThrow(_) => ExpressionNode {
                id,
                expression: Self::node_to_json_string(node),
                children: vec![],
            },

            CompiledNode::CompiledMissing(_) | CompiledNode::CompiledMissingSome(_) => {
                ExpressionNode {
                    id,
                    expression: Self::node_to_json_string(node),
                    children: vec![],
                }
            }
        }
    }

    /// Check if a node is an operator (not a literal value)
    fn is_operator_node(node: &CompiledNode) -> bool {
        !matches!(node, CompiledNode::Value { .. })
    }

    /// Convert a CompiledNode to its JSON string representation
    fn node_to_json_string(node: &CompiledNode) -> String {
        match node {
            CompiledNode::Value { value, .. } => value.to_string(),
            CompiledNode::Array { nodes, .. } => {
                let items: Vec<String> = nodes.iter().map(Self::node_to_json_string).collect();
                format!("[{}]", items.join(", "))
            }
            CompiledNode::BuiltinOperator { opcode, args, .. } => {
                Self::builtin_to_json_string(opcode, args)
            }
            CompiledNode::CustomOperator(data) => {
                Self::custom_to_json_string(&data.name, &data.args)
            }
            #[cfg(feature = "preserve")]
            CompiledNode::StructuredObject(data) => Self::structured_to_json_string(&data.fields),
            CompiledNode::CompiledVar {
                scope_level,
                segments,
                default_value,
                ..
            } => {
                Self::compiled_var_to_json_string(*scope_level, segments, default_value.as_deref())
            }
            #[cfg(feature = "ext-control")]
            CompiledNode::CompiledExists(data) => {
                Self::compiled_exists_to_json_string(data.scope_level, &data.segments)
            }
            #[cfg(feature = "ext-string")]
            CompiledNode::CompiledSplitRegex(data) => {
                format!(
                    "{{\"split\": [{}, \"{}\"]}}",
                    Self::node_to_json_string(&data.args[0]),
                    data.regex.as_str()
                )
            }
            #[cfg(feature = "error-handling")]
            CompiledNode::CompiledThrow(data) => {
                if let serde_json::Value::Object(err_map) = &data.error
                    && let Some(serde_json::Value::String(s)) = err_map.get("type")
                {
                    return format!("{{\"throw\": \"{}\"}}", s);
                }
                format!("{{\"throw\": {}}}", data.error)
            }
            CompiledNode::CompiledMissing(data) => {
                let parts: Vec<String> = data
                    .args
                    .iter()
                    .map(|a| match a {
                        crate::node::CompiledMissingArg::Static { path, .. } => {
                            format!("\"{}\"", path)
                        }
                        crate::node::CompiledMissingArg::Dynamic(n) => Self::node_to_json_string(n),
                    })
                    .collect();
                format!("{{\"missing\": [{}]}}", parts.join(", "))
            }
            CompiledNode::CompiledMissingSome(data) => {
                let min_str = match &data.min_present {
                    crate::node::CompiledMissingMin::Static(n) => n.to_string(),
                    crate::node::CompiledMissingMin::Dynamic(n) => Self::node_to_json_string(n),
                };
                let paths_str = match &data.paths {
                    crate::node::CompiledMissingPaths::Static(paths) => {
                        let items: Vec<String> =
                            paths.iter().map(|(p, _)| format!("\"{}\"", p)).collect();
                        format!("[{}]", items.join(", "))
                    }
                    crate::node::CompiledMissingPaths::Dynamic(n) => Self::node_to_json_string(n),
                };
                format!("{{\"missing_some\": [{}, {}]}}", min_str, paths_str)
            }
        }
    }

    fn builtin_to_json_string(opcode: &OpCode, args: &[CompiledNode]) -> String {
        let op_str = opcode.as_str();
        let args_str = if args.len() == 1 {
            Self::node_to_json_string(&args[0])
        } else {
            let items: Vec<String> = args.iter().map(Self::node_to_json_string).collect();
            format!("[{}]", items.join(", "))
        };
        format!("{{\"{}\": {}}}", op_str, args_str)
    }

    fn custom_to_json_string(name: &str, args: &[CompiledNode]) -> String {
        let args_str = if args.len() == 1 {
            Self::node_to_json_string(&args[0])
        } else {
            let items: Vec<String> = args.iter().map(Self::node_to_json_string).collect();
            format!("[{}]", items.join(", "))
        };
        format!("{{\"{}\": {}}}", name, args_str)
    }

    #[cfg(feature = "preserve")]
    fn structured_to_json_string(fields: &[(String, CompiledNode)]) -> String {
        let items: Vec<String> = fields
            .iter()
            .map(|(key, node)| format!("\"{}\": {}", key, Self::node_to_json_string(node)))
            .collect();
        format!("{{{}}}", items.join(", "))
    }

    fn compiled_var_to_json_string(
        scope_level: u32,
        segments: &[crate::node::PathSegment],
        default_value: Option<&CompiledNode>,
    ) -> String {
        use crate::node::PathSegment;
        if scope_level == 0 {
            let path: String = segments
                .iter()
                .map(|seg| match seg {
                    PathSegment::Field(s) | PathSegment::FieldOrIndex(s, _) => s.to_string(),
                    PathSegment::Index(i) => i.to_string(),
                })
                .collect::<Vec<_>>()
                .join(".");
            match default_value {
                Some(def) => {
                    format!(
                        "{{\"var\": [\"{}\", {}]}}",
                        path,
                        Self::node_to_json_string(def)
                    )
                }
                None => format!("{{\"var\": \"{}\"}}", path),
            }
        } else {
            let mut parts = vec![format!("[{}]", scope_level)];
            for seg in segments {
                match seg {
                    PathSegment::Field(s) | PathSegment::FieldOrIndex(s, _) => {
                        parts.push(format!("\"{}\"", s))
                    }
                    PathSegment::Index(i) => parts.push(i.to_string()),
                }
            }
            format!("{{\"val\": [{}]}}", parts.join(", "))
        }
    }

    #[cfg(feature = "ext-control")]
    fn compiled_exists_to_json_string(
        _scope_level: u32,
        segments: &[crate::node::PathSegment],
    ) -> String {
        use crate::node::PathSegment;
        if segments.len() == 1 {
            match &segments[0] {
                PathSegment::Field(s) | PathSegment::FieldOrIndex(s, _) => {
                    format!("{{\"exists\": \"{}\"}}", s)
                }
                PathSegment::Index(i) => format!("{{\"exists\": {}}}", i),
            }
        } else {
            let parts: Vec<String> = segments
                .iter()
                .map(|seg| match seg {
                    PathSegment::Field(s) | PathSegment::FieldOrIndex(s, _) => {
                        format!("\"{}\"", s)
                    }
                    PathSegment::Index(i) => i.to_string(),
                })
                .collect();
            format!("{{\"exists\": [{}]}}", parts.join(", "))
        }
    }
}

/// Captures state at each evaluation step.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExecutionStep {
    /// Sequential step number
    pub id: u32,
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
pub struct TraceCollector {
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
            id: self.step_counter,
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
            id: self.step_counter,
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::OpCode;

    #[test]
    fn test_expression_node_from_simple_operator() {
        // Create a simple {"var": "age"} node
        let node = CompiledNode::BuiltinOperator {
            id: crate::node::SYNTHETIC_ID,
            opcode: OpCode::Var,
            args: vec![CompiledNode::synthetic_value(serde_json::json!("age"))].into_boxed_slice(),
        };

        let tree = ExpressionNode::build_from_compiled(&node);

        // Synthetic test nodes all share SYNTHETIC_ID (0); the structural
        // assertions below still hold.
        assert_eq!(tree.id, crate::node::SYNTHETIC_ID);
        assert_eq!(tree.expression, r#"{"var": "age"}"#);
        assert!(tree.children.is_empty()); // "age" is a literal, not a child
    }

    #[test]
    fn test_expression_node_from_nested_operator() {
        // Create {">=": [{"var": "age"}, 18]}
        let var_node = CompiledNode::BuiltinOperator {
            id: crate::node::SYNTHETIC_ID,
            opcode: OpCode::Var,
            args: vec![CompiledNode::synthetic_value(serde_json::json!("age"))].into_boxed_slice(),
        };
        let node = CompiledNode::BuiltinOperator {
            id: crate::node::SYNTHETIC_ID,
            opcode: OpCode::GreaterThanEqual,
            args: vec![
                var_node,
                CompiledNode::synthetic_value(serde_json::json!(18)),
            ]
            .into_boxed_slice(),
        };

        let tree = ExpressionNode::build_from_compiled(&node);

        assert_eq!(tree.id, crate::node::SYNTHETIC_ID);
        assert!(tree.expression.contains(">="));
        assert_eq!(tree.children.len(), 1); // var node is a child
        assert!(tree.children[0].expression.contains("var"));
    }

    #[test]
    fn test_trace_collector_records_steps() {
        let mut collector = TraceCollector::new();

        collector.record_step(0, serde_json::json!({"age": 25}), serde_json::json!(25));
        collector.record_step(1, serde_json::json!({"age": 25}), serde_json::json!(true));

        let steps = collector.into_steps();
        assert_eq!(steps.len(), 2);
        assert_eq!(steps[0].id, 0);
        assert_eq!(steps[0].node_id, 0);
        assert_eq!(steps[1].id, 1);
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
}
