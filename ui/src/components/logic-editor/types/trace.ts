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
 * Mirrors the Rust `StructuredError` serialization — `type` is a stable
 * machine-readable kind, `message` is human-readable, and variant-specific
 * extras appear as extra fields.
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
}

export interface TracedResult {
  result: unknown;
  expression_tree: ExpressionNode;
  steps: ExecutionStep[];
  error?: string;
  error_structured?: StructuredError;
}
