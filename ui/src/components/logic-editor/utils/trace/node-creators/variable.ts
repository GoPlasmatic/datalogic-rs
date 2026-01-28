import type {
  LogicNode,
  JsonLogicValue,
  VariableNodeData,
} from '../../../types';
import type { ExpressionNode } from '../../../types/trace';
import type { ParentInfo } from '../../converters/types';
import type { TraceContext } from '../types';
import { createArgEdge } from '../../node-factory';
import { mapInlinedChildren } from '../inline-mapping';

/**
 * Create a variable node from trace data
 */
export function createVariableNodeFromTrace(
  nodeId: string,
  expression: JsonLogicValue,
  children: ExpressionNode[],
  context: TraceContext,
  parentInfo: ParentInfo
): void {
  // Map any children to this node (e.g., nested default value expressions)
  if (children && children.length > 0) {
    mapInlinedChildren(children, nodeId, context.traceNodeMap);
  }

  const obj = expression as Record<string, unknown>;
  const operator = Object.keys(obj)[0] as 'var' | 'val' | 'exists';
  const operands = obj[operator];

  let path: string;
  let defaultValue: JsonLogicValue | undefined;

  if (Array.isArray(operands)) {
    path = String(operands[0] ?? '');
    defaultValue = operands[1] as JsonLogicValue | undefined;
  } else {
    path = String(operands ?? '');
  }

  const node: LogicNode = {
    id: nodeId,
    type: 'variable',
    position: { x: 0, y: 0 },
    data: {
      type: 'variable',
      operator,
      path,
      defaultValue,
      expression,
      parentId: parentInfo.parentId,
      argIndex: parentInfo.argIndex,
      branchType: parentInfo.branchType,
    } as VariableNodeData,
  };
  context.nodes.push(node);

  // Add edge from parent if exists and not a branch type
  if (parentInfo.parentId && !parentInfo.branchType) {
    context.edges.push(createArgEdge(parentInfo.parentId, nodeId, parentInfo.argIndex ?? 0));
  }
}
