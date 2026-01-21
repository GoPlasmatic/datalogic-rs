# Proposal: Execution Trace for Step-by-Step Debugging

## Overview

Add execution tracing to datalogic-rs for Web UI debugging. A new `evaluate_json_with_trace` function returns both the result and a complete execution trace that enables step-by-step replay in the UI.

---

## API Design

### New Function

```rust
impl DataLogic {
    /// Evaluate JSON logic with execution trace for debugging
    pub fn evaluate_json_with_trace(
        &self,
        logic: &str,
        data: &str,
    ) -> Result<TracedResult>
}
```

### Response Structure

```rust
pub struct TracedResult {
    /// The evaluation result
    pub result: Value,
    /// Expression tree with unique IDs
    pub expression_tree: ExpressionNode,
    /// Ordered execution steps
    pub steps: Vec<ExecutionStep>,
}
```

---

## Data Structures

### ExpressionNode

Represents the structure of the logic for flow diagram rendering.

```rust
pub struct ExpressionNode {
    /// Unique identifier for this node
    pub id: u32,
    /// JSON string of this sub-expression
    pub expression: String,
    /// Child nodes (arguments/operands)
    pub children: Vec<ExpressionNode>,
}
```

**Example:**

For logic `{"and": [{">=": [{"var": "age"}, 18]}, true]}`:

```json
{
  "id": 0,
  "expression": "{\"and\": [{\">=\": [{\"var\": \"age\"}, 18]}, true]}",
  "children": [
    {
      "id": 1,
      "expression": "{\">=\": [{\"var\": \"age\"}, 18]}",
      "children": [
        {
          "id": 2,
          "expression": "{\"var\": \"age\"}",
          "children": []
        }
      ]
    }
  ]
}
```

**Note:** Literal values (`18`, `true`, `"age"`) are not represented as separate nodes since they don't generate execution steps.

### ExecutionStep

Captures state at each evaluation step.

```rust
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
    pub iteration_index: Option<u32>,
    /// Total iteration count (only for iterator body evaluations)
    pub iteration_total: Option<u32>,
}
```

**Example steps for the above logic with data `{"age": 25}`:**

```json
[
  {"id": 0, "node_id": 2, "context": {"age": 25}, "result": 25, "error": null},
  {"id": 1, "node_id": 1, "context": {"age": 25}, "result": true, "error": null},
  {"id": 2, "node_id": 0, "context": {"age": 25}, "result": true, "error": null}
]
```

---

## Design Decisions

### Literals Are Not Stepped

Primitive values (`18`, `"age"`, `true`, `null`, etc.) do not generate execution steps. Only operator expressions produce steps.

### Context Shows Current Scope Only

The `context` field shows only the current scope (what `{"var": ""}` would resolve to), not the full context stack.

### Lazy/Short-Circuit Evaluation

Steps only include nodes that were actually evaluated. For example:
- `{"and": [false, {"expensive": "op"}]}` — second operand is never evaluated, so no step for it
- `{"or": [true, {"other": "op"}]}` — second operand is skipped

### Iteration Operators

For `map`, `filter`, `reduce`, the same `node_id` may appear multiple times in steps with different contexts and results:

```json
{"map": [[1, 2, 3], {"*": [{"var": ""}, 2]}]}
```

Steps:
```json
[
  {"id": 0, "node_id": 2, "context": 1, "result": 2, "error": null, "iteration_index": 0, "iteration_total": 3},
  {"id": 1, "node_id": 2, "context": 2, "result": 4, "error": null, "iteration_index": 1, "iteration_total": 3},
  {"id": 2, "node_id": 2, "context": 3, "result": 6, "error": null, "iteration_index": 2, "iteration_total": 3},
  {"id": 3, "node_id": 1, "context": [1, 2, 3], "result": [2, 4, 6], "error": null, "iteration_index": null, "iteration_total": null}
]
```

### Error Handling

When evaluation fails, the step includes an `error` field and `result` is `null`:

```json
{"id": 3, "node_id": 1, "context": {"x": "not a number"}, "result": null, "error": "Cannot perform arithmetic on string"}
```

Partial steps up to the error point are included in the trace.

---

## WASM Binding

```rust
#[wasm_bindgen]
pub fn evaluate_with_trace(logic: &str, data: &str) -> Result<String, String>
```

Returns JSON string of `TracedResult`.

---

## UI Integration

The Web UI can:

1. **Render flow diagram** from `expression_tree` using node IDs and children relationships
2. **Step through execution** by iterating through `steps` array
3. **Highlight current node** by matching `step.node_id` to `expression_tree.id`
4. **Show context and result** for each step
5. **Parse expression strings** as needed for detailed display
6. **Handle repeated node execution** (same node_id, different context) for iteration operators
