import type { TracedResult, StructuredError } from '../../types/trace';
import { traceIdToNodeId } from './trace-ids';

/**
 * A trace-level failure: the structured error when the engine emitted one,
 * otherwise the plain message.
 */
export type TraceFailure = StructuredError | string;

/**
 * True when the engine could not build an expression tree at all (parse
 * error, unknown operator, ...): the envelope carries the placeholder tree
 * `{ id: 0, expression: '', children: [] }` and no steps.
 */
export function isCompileFailedTrace(trace: TracedResult): boolean {
  const tree = trace.expression_tree;
  if (!tree) return true;
  return tree.expression === '' && (tree.children?.length ?? 0) === 0;
}

/**
 * The failure reported by a trace envelope, or undefined when evaluation
 * succeeded. Prefers `structured_error` (machine-readable, with the
 * `node_ids` breadcrumb) over the flat `error` message.
 */
export function getTraceFailure(trace: TracedResult): TraceFailure | undefined {
  if (trace.structured_error) return trace.structured_error;
  if (typeof trace.error === 'string' && trace.error.length > 0) return trace.error;
  return undefined;
}

/** Human-readable message for a trace failure. */
export function formatTraceFailure(failure: TraceFailure): string {
  return typeof failure === 'string' ? failure : failure.message;
}

/** Machine-readable kind of a trace failure (undefined for plain messages). */
export function traceFailureType(failure: TraceFailure): string | undefined {
  return typeof failure === 'string' ? undefined : failure.type;
}

/**
 * Visual node ids on the engine's failure breadcrumb (`structured_error.node_ids`,
 * innermost first), resolved through the trace node map. Ids the map does not
 * know are dropped. Returns an empty set when there is no breadcrumb.
 */
export function resolveFailedNodeIds(
  trace: TracedResult,
  traceNodeMap: Map<string, string>
): Set<string> {
  const ids = new Set<string>();
  const nodeIds = trace.structured_error?.node_ids;
  if (!nodeIds) return ids;
  for (const id of nodeIds) {
    const visualId = traceNodeMap.get(traceIdToNodeId(id));
    if (visualId) ids.add(visualId);
  }
  return ids;
}
