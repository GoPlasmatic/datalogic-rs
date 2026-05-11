import type { ExpressionNode } from '../../types/trace';
import { traceIdToNodeId } from './evaluation-results';

/**
 * Map all children of an expression node to a parent visual node ID (for inlined children)
 */
export function mapInlinedChildren(
  children: ExpressionNode[],
  parentVisualId: string,
  traceNodeMap: Map<string, string>
): void {
  for (const child of children) {
    const traceId = traceIdToNodeId(child.id);
    traceNodeMap.set(traceId, parentVisualId);
    // Also recursively map any nested children
    if (child.children && child.children.length > 0) {
      mapInlinedChildren(child.children, parentVisualId, traceNodeMap);
    }
  }
}
