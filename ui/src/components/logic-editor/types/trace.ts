// Trace API types from the WASM evaluate_with_trace function

export interface ExpressionNode {
  id: number;
  expression: string;  // JSON string of sub-expression
  children: ExpressionNode[];
}

export interface ExecutionStep {
  id: number;
  node_id: number;
  context: unknown;
  result?: unknown;
  error?: string;
  iteration_index?: number;
  iteration_total?: number;
}

/**
 * Structured error shape emitted by the WASM `*_structured` entry points.
 *
 * Mirrors the Rust `Error` serialization — `type` is a stable
 * machine-readable kind, `message` is human-readable, and variant-specific
 * extras appear as extra fields. `node_ids` is the v5 failure breadcrumb
 * (compile-time node IDs, root-to-leaf); `stage` is set on parse-time
 * failures from the WASM boundary itself.
 */
export interface StructuredError {
  type: string;
  message: string;
  operator?: string;
  variable?: string;
  level?: number;
  thrown?: unknown;
  index?: number;
  length?: number;
  stage?: string;
  node_ids?: number[];
}

export interface TracedResult {
  result: unknown;
  expression_tree: ExpressionNode;
  steps: ExecutionStep[];
  error?: string;
  structured_error?: StructuredError;
}
