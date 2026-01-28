import type { TracedResult } from '../../types/trace';
import type { ValueType } from './types';

/**
 * Convert trace node ID to string node ID
 */
export function traceIdToNodeId(id: number): string {
  return `trace-${id}`;
}

/**
 * Build evaluation results map from trace execution steps
 */
export function buildEvaluationResultsFromTrace(
  trace: TracedResult
): Map<string, { value: unknown; error: string | null; type: ValueType }> {
  const results = new Map<string, { value: unknown; error: string | null; type: ValueType }>();

  if (!trace.steps) {
    return results;
  }

  for (const step of trace.steps) {
    const nodeId = traceIdToNodeId(step.node_id);

    // Determine the value type
    let valueType: ValueType = 'undefined';
    const value = step.result;
    if (value === null) valueType = 'null';
    else if (value === undefined) valueType = 'undefined';
    else if (Array.isArray(value)) valueType = 'array';
    else if (typeof value === 'boolean') valueType = 'boolean';
    else if (typeof value === 'number') valueType = 'number';
    else if (typeof value === 'string') valueType = 'string';
    else if (typeof value === 'object') valueType = 'object';

    results.set(nodeId, {
      value: step.result,
      error: step.error ?? null,
      type: valueType,
    });
  }

  return results;
}
