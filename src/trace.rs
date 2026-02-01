//! Execution tracing for step-by-step debugging.
//!
//! This module provides execution tracing capabilities for debugging JSONLogic
//! expressions. It generates an expression tree with unique IDs and records
//! each evaluation step for replay in the Web UI.

use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::HashMap;

use crate::{CompiledNode, OpCode};

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
    /// Build an expression tree from a CompiledNode, assigning unique IDs.
    ///
    /// Returns the expression tree and a mapping from node pointers to IDs.
    pub fn build_from_compiled(node: &CompiledNode) -> (ExpressionNode, HashMap<usize, u32>) {
        let mut id_counter = 0u32;
        let mut node_id_map = HashMap::new();
        let tree = Self::build_node(node, &mut id_counter, &mut node_id_map);
        (tree, node_id_map)
    }

    fn build_node(
        node: &CompiledNode,
        id_counter: &mut u32,
        node_id_map: &mut HashMap<usize, u32>,
    ) -> ExpressionNode {
        let id = *id_counter;
        *id_counter += 1;

        // Store the mapping from node pointer to ID
        let node_ptr = node as *const CompiledNode as usize;
        node_id_map.insert(node_ptr, id);

        match node {
            CompiledNode::Value { value, .. } => {
                // Literals don't have children but we still need to represent them
                // in the tree for completeness
                ExpressionNode {
                    id,
                    expression: value.to_string(),
                    children: vec![],
                }
            }
            CompiledNode::Array { nodes, .. } => {
                let children: Vec<ExpressionNode> = nodes
                    .iter()
                    .filter(|n| Self::is_operator_node(n))
                    .map(|n| Self::build_node(n, id_counter, node_id_map))
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
                    .map(|n| Self::build_node(n, id_counter, node_id_map))
                    .collect();

                ExpressionNode {
                    id,
                    expression: Self::builtin_to_json_string(opcode, args),
                    children,
                }
            }
            CompiledNode::CustomOperator { name, args, .. } => {
                let children: Vec<ExpressionNode> = args
                    .iter()
                    .filter(|n| Self::is_operator_node(n))
                    .map(|n| Self::build_node(n, id_counter, node_id_map))
                    .collect();

                ExpressionNode {
                    id,
                    expression: Self::custom_to_json_string(name, args),
                    children,
                }
            }
            CompiledNode::StructuredObject { fields, .. } => {
                let children: Vec<ExpressionNode> = fields
                    .iter()
                    .filter(|(_, n)| Self::is_operator_node(n))
                    .map(|(_, n)| Self::build_node(n, id_counter, node_id_map))
                    .collect();

                ExpressionNode {
                    id,
                    expression: Self::structured_to_json_string(fields),
                    children,
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
            CompiledNode::CustomOperator { name, args, .. } => {
                Self::custom_to_json_string(name, args)
            }
            CompiledNode::StructuredObject { fields, .. } => {
                Self::structured_to_json_string(fields)
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

    fn structured_to_json_string(fields: &[(String, CompiledNode)]) -> String {
        let items: Vec<String> = fields
            .iter()
            .map(|(key, node)| format!("\"{}\": {}", key, Self::node_to_json_string(node)))
            .collect();
        format!("{{{}}}", items.join(", "))
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
            opcode: OpCode::Var,
            args: vec![CompiledNode::Value {
                value: serde_json::json!("age"),
            }],
        };

        let (tree, node_id_map) = ExpressionNode::build_from_compiled(&node);

        assert_eq!(tree.id, 0);
        assert_eq!(tree.expression, r#"{"var": "age"}"#);
        assert!(tree.children.is_empty()); // "age" is a literal, not a child
        assert_eq!(node_id_map.len(), 1);
    }

    #[test]
    fn test_expression_node_from_nested_operator() {
        // Create {">=": [{"var": "age"}, 18]}
        let var_node = CompiledNode::BuiltinOperator {
            opcode: OpCode::Var,
            args: vec![CompiledNode::Value {
                value: serde_json::json!("age"),
            }],
        };
        let node = CompiledNode::BuiltinOperator {
            opcode: OpCode::GreaterThanEqual,
            args: vec![
                var_node,
                CompiledNode::Value {
                    value: serde_json::json!(18),
                },
            ],
        };

        let (tree, node_id_map) = ExpressionNode::build_from_compiled(&node);

        assert_eq!(tree.id, 0);
        assert!(tree.expression.contains(">="));
        assert_eq!(tree.children.len(), 1); // var node is a child
        assert_eq!(tree.children[0].id, 1);
        assert!(tree.children[0].expression.contains("var"));
        assert_eq!(node_id_map.len(), 2);
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
