// Trace API types from the WASM evaluate_with_trace function

export interface ExpressionNode {
  id: number;
  expression: string;  // JSON string of sub-expression (engine-canonicalized)
  children: ExpressionNode[];
}

/**
 * One recorded evaluation step. Mirrors the Rust `ExecutionStep`: `result`
 * and `error` are always emitted (null when absent), the iteration fields
 * only inside iterator operators (map / filter / reduce / all / some / none).
 */
export interface ExecutionStep {
  step_id: number;
  node_id: number;
  context: unknown;
  result: unknown;
  error: string | null;
  iteration_index?: number;
  iteration_total?: number;
}

/**
 * Structured error shape emitted by the WASM `*_structured` entry points.
 *
 * Mirrors the Rust `Error` serialization: `type` is a stable
 * machine-readable kind, `message` is human-readable, and variant-specific
 * extras appear as extra fields. `node_ids` is the v5 failure breadcrumb
 * (compile-time node IDs, innermost first); `stage` is set on parse-time
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
  /** `BudgetExceeded`: the operation ceiling that was crossed. */
  budget?: number;
  /** `BudgetExceeded`: operations charged when the crossing was detected. */
  spent?: number;
  node_ids?: number[];
  /** `CompileError` (from `Engine#compileChecked`): every diagnostic that failed compilation. */
  diagnostics?: RuleDiagnostic[];
}

/**
 * Severity of a static rule diagnostic emitted by `Engine#check` or `Engine#compileChecked`.
 */
export type DiagnosticSeverity = 'error' | 'warning';

/**
 * Machine-readable diagnostic code emitted by `Engine#check`.
 */
export type DiagnosticCode =
  | 'Unparsable'
  | 'UnknownOperator'
  | 'NotAnOperator'
  | 'ArgumentForm'
  | 'ArgumentCount'
  | 'InvalidTimezone'
  | 'SimilarToOperator'
  | 'Compile'
  | 'OperatorCheck';

/**
 * One static rule diagnostic returned by `Engine#check`.
 */
export interface RuleDiagnostic {
  code: DiagnosticCode;
  severity: DiagnosticSeverity;
  message: string;
  /** RFC 6901 JSON Pointer into the rule (`""` for the root). */
  pointer: string;
  /** Operator name when the diagnostic applies to a specific operator call, or `null`. */
  operator: string | null;
}

/**
 * Segment of a static data path reported by `Rule#facts`.
 * Strings are object keys; numbers are array indices.
 */
export type DataPathSegment = string | number;

/**
 * Static analysis of a compiled rule returned by `Rule#facts`.
 */
export interface RuleFacts {
  /** Static data paths read by the rule, sorted and deduplicated. `[]` is the root (`{"var": ""}`). */
  reads: DataPathSegment[][];
  /** `true` when the rule computes a data path at runtime or calls a context-reading custom operator. */
  computed_reads: boolean;
  /** `true` when `reads` is a complete upper bound on what the rule can read from the context. */
  reads_complete: boolean;
  /** `true` when the rule reads the data context at all. */
  reads_data: boolean;
  /** Built-in operators used in the rule (canonical names, sorted). */
  operators: string[];
  /** Custom operators used in the rule (sorted). */
  custom_operators: string[];
  /** `true` when the rule always returns the same output for the same input. */
  deterministic: boolean;
}

/**
 * Metadata for one built-in operator returned by `Engine#operators`.
 */
export interface WasmOperatorInfo {
  name: string;
  aliases: string[];
  family: string;
  feature: string | null;
  min_args: number;
  max_args: number | null;
  reads_context: boolean;
  effect: 'Pure' | 'Clock' | 'Random' | 'Throw';
  cost: number;
  scoped_arg: {
    scope_index: number;
    body_indices: number[];
    shape: string;
    metadata: string[];
  } | null;
}

/**
 * Envelope returned by `evaluateWithTrace`. On a compile-stage failure
 * (parse error, unknown operator) `expression_tree` is the placeholder
 * `{ id: 0, expression: '', children: [] }` and `steps` is empty; `error` and
 * `structured_error` are present whenever evaluation did not produce a result.
 */
export interface TracedResult {
  result: unknown;
  expression_tree: ExpressionNode;
  steps: ExecutionStep[];
  error?: string;
  structured_error?: StructuredError;
  /**
   * The RFC 6901 JSON Pointer into the rule of the value each node id was
   * compiled from, keyed by the id as a decimal string. Absent when the rule
   * did not compile.
   */
  pointers?: Record<string, string>;
}
