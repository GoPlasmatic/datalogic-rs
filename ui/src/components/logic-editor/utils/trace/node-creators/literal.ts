import type {
  LogicNode,
  JsonLogicValue,
  LiteralNodeData,
} from '../../../types';
import type { ExpressionNode } from '../../../types/trace';
import type { ParentInfo } from '../../converters/types';
import type { TraceContext } from '../types';
import { getValueType } from '../../type-helpers';
import { createArgEdge } from '../../node-factory';
import { mapInlinedChildren } from '../inline-mapping';

/**
 * Create a literal node from trace data
 */
export function createLiteralNodeFromTrace(
  nodeId: string,
  value: JsonLogicValue,
  children: ExpressionNode[],
  context: TraceContext,
  parentInfo: ParentInfo
): void {
  // Map any children to this node (shouldn't happen for literals, but be safe)
  if (children && children.length > 0) {
    mapInlinedChildren(children, nodeId, context.traceNodeMap);
  }

  const node: LogicNode = {
    id: nodeId,
    type: 'literal',
    position: { x: 0, y: 0 },
    data: {
      type: 'literal',
      value,
      valueType: getValueType(value),
      expression: value,
      parentId: parentInfo.parentId,
      argIndex: parentInfo.argIndex,
      branchType: parentInfo.branchType,
    } as LiteralNodeData,
  };
  context.nodes.push(node);

  // Add edge from parent if exists and not a branch type
  if (parentInfo.parentId && !parentInfo.branchType) {
    context.edges.push(createArgEdge(parentInfo.parentId, nodeId, parentInfo.argIndex ?? 0));
  }
}
