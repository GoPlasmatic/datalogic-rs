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

export interface TracedResult {
  result: unknown;
  expression_tree: ExpressionNode;
  steps: ExecutionStep[];
  error?: string;
}
