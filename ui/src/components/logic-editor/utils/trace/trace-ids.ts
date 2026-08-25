/**
 * Trace node ids. The engine numbers expression-tree nodes; the editor keys
 * its own maps by the string form.
 */
export function traceIdToNodeId(id: number): string {
  return `trace-${id}`;
}
