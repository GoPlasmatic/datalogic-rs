import type {
  LogicNode,
  JsonLogicValue,
  OperatorNodeData,
} from '../../../types';
import type { ExpressionNode } from '../../../types/trace';
import type { ParentInfo } from '../../converters/types';
import type { TraceContext } from '../types';
import { getOperatorMeta, getOperatorTitle } from '../../../constants';
import { generateExpressionText } from '../../formatting';
import { isSimpleOperand } from '../../type-helpers';
import { createArgEdge } from '../../node-factory';
import { findMatchingChild } from '../child-matching';
import { mapInlinedChildren } from '../inline-mapping';
import { traceIdToNodeId } from '../evaluation-results';

// Forward declaration for processExpressionNode (will be passed in)
type ProcessExpressionNodeFn = (
  exprNode: ExpressionNode,
  context: TraceContext,
  parentInfo: ParentInfo,
  originalExpression?: JsonLogicValue
) => string;

/**
 * Create an operator node (single-arg or unary) from trace data
 */
export function createOperatorNodeFromTrace(
  nodeId: string,
  expression: JsonLogicValue,
  children: ExpressionNode[],
  context: TraceContext,
  parentInfo: ParentInfo,
  processExpressionNode: ProcessExpressionNodeFn
): void {
  const obj = expression as Record<string, unknown>;
  const operator = Object.keys(obj)[0];
  const operands = obj[operator];
  const operandArray: JsonLogicValue[] = Array.isArray(operands) ? operands : [operands];

  const meta = getOperatorMeta(operator);
  const expressionText = generateExpressionText(expression);
  const childIds: string[] = [];

  // For unary operators with simple operands, show inline
  const singleOperand = operandArray[0];
  const isUnaryWithSimpleArg = operandArray.length === 1 && isSimpleOperand(singleOperand);

  if (isUnaryWithSimpleArg) {
    // Map the simple operand child to this node (since it's inlined)
    const match = findMatchingChild(singleOperand, children, new Set());
    if (match) {
      const traceId = traceIdToNodeId(match.child.id);
      context.traceNodeMap.set(traceId, nodeId);
      // Also map any nested children
      if (match.child.children && match.child.children.length > 0) {
        mapInlinedChildren(match.child.children, nodeId, context.traceNodeMap);
      }
    }

    // Create inline operator node
    const node: LogicNode = {
      id: nodeId,
      type: 'operator',
      position: { x: 0, y: 0 },
      data: {
        type: 'operator',
        operator,
        category: meta.category,
        label: getOperatorTitle(operator),
        childIds: [],
        collapsed: false,
        expressionText,
        expression,
        inlineDisplay: expressionText,
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
    return;
  }

  // Process children - match by expression content
  const usedChildIndices = new Set<number>();
  operandArray.forEach((operand, idx) => {
    if (!isSimpleOperand(operand)) {
      const match = findMatchingChild(operand, children, usedChildIndices);
      if (match) {
        usedChildIndices.add(match.index);
        const childId = processExpressionNode(match.child, context, {
          parentId: nodeId,
          argIndex: idx,
        });
        childIds.push(childId);
      }
    } else {
      // Simple operand is inlined - map the trace child to this parent node
      const match = findMatchingChild(operand, children, usedChildIndices);
      if (match) {
        usedChildIndices.add(match.index);
        const traceId = traceIdToNodeId(match.child.id);
        context.traceNodeMap.set(traceId, nodeId);
        // Also map any nested children
        if (match.child.children && match.child.children.length > 0) {
          mapInlinedChildren(match.child.children, nodeId, context.traceNodeMap);
        }
      }
    }
  });

  const node: LogicNode = {
    id: nodeId,
    type: 'operator',
    position: { x: 0, y: 0 },
    data: {
      type: 'operator',
      operator,
      category: meta.category,
      label: getOperatorTitle(operator),
      childIds,
      collapsed: false,
      expressionText,
      expression,
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
