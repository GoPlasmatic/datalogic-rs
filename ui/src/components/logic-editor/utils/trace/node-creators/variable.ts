import type {
  LogicNode,
  JsonLogicValue,
  OperatorNodeData,
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

  // Build cells for the variable operator
  const cells: { type: 'editable'; fieldId: string; fieldType: 'text'; value: unknown; placeholder: string; label: string; index: number }[] = [
    { type: 'editable', fieldId: 'path', fieldType: 'text', value: path, placeholder: 'path', label: path || '', index: 0 },
  ];
  if (defaultValue !== undefined) {
    cells.push({ type: 'editable', fieldId: 'default', fieldType: 'text', value: defaultValue, placeholder: 'default', label: String(defaultValue), index: 1 });
  }

  const node: LogicNode = {
    id: nodeId,
    type: 'operator',
    position: { x: 0, y: 0 },
    data: {
      type: 'operator',
      operator,
      category: 'variable',
      label: operator,
      icon: 'database',
      cells,
      expression,
      path,
      defaultValue,
      parentId: parentInfo.parentId,
      argIndex: parentInfo.argIndex,
      branchType: parentInfo.branchType,
    } as OperatorNodeData,
  };
  context.nodes.push(node);

  // Add edge from parent if exists and not a branch type
  if (parentInfo.parentId && !parentInfo.branchType) {
    context.edges.push(createArgEdge(parentInfo.parentId, nodeId, parentInfo.argIndex ?? 0));
  }
}
