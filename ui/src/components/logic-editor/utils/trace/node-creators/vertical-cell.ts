import type {
  LogicNode,
  JsonLogicValue,
  CellData,
  VerticalCellNodeData,
} from '../../../types';
import type { ExpressionNode } from '../../../types/trace';
import type { ParentInfo } from '../../converters/types';
import type { TraceContext } from '../types';
import { getOperatorMeta, getOperatorTitle, TRUNCATION_LIMITS } from '../../../constants';
import { CATEGORY_ICONS, ITERATOR_ARG_ICONS, getOperandTypeIcon, CONTROL_ICONS, type IconName } from '../../icons';
import { generateExpressionText, generateArgSummary, formatOperandLabel } from '../../formatting';
import { isSimpleOperand } from '../../type-helpers';
import { createBranchEdge, createArgEdge } from '../../node-factory';
import { findMatchingChild } from '../child-matching';
import { mapInlinedChildren } from '../inline-mapping';
import { traceIdToNodeId } from '../evaluation-results';

// Forward declaration for processExpressionNode and createFallbackNode
type ProcessExpressionNodeFn = (
  exprNode: ExpressionNode,
  context: TraceContext,
  parentInfo: ParentInfo,
  originalExpression?: JsonLogicValue
) => string;

type CreateFallbackNodeFn = (
  nodeId: string,
  value: JsonLogicValue,
  context: TraceContext,
  parentInfo: ParentInfo
) => void;

/**
 * Create a vertical cell node for multi-arg operators from trace data
 */
export function createVerticalCellNodeFromTrace(
  nodeId: string,
  expression: JsonLogicValue,
  children: ExpressionNode[],
  context: TraceContext,
  parentInfo: ParentInfo,
  processExpressionNode: ProcessExpressionNodeFn,
  createFallbackNode: CreateFallbackNodeFn
): void {
  const obj = expression as Record<string, unknown>;
  const operator = Object.keys(obj)[0];
  const operands = obj[operator];
  const operandArray: JsonLogicValue[] = Array.isArray(operands) ? operands : [operands];

  const meta = getOperatorMeta(operator);
  const cells: CellData[] = [];
  let branchIndex = 0;
  const usedChildIndices = new Set<number>();

  // Determine icon
  let icon: IconName = CATEGORY_ICONS[meta.category] || 'list';
  if (operator === 'or') icon = CONTROL_ICONS.orOperator;

  const iteratorIcons = ITERATOR_ARG_ICONS[operator];

  operandArray.forEach((operand, idx) => {
    const typeIcon = getOperandTypeIcon(operand as JsonLogicValue);
    const cellIcon = iteratorIcons ? iteratorIcons[idx] || typeIcon : typeIcon;

    if (isSimpleOperand(operand as JsonLogicValue)) {
      // Simple operand is inlined - map the trace child to this parent node
      const match = findMatchingChild(operand as JsonLogicValue, children, usedChildIndices);
      if (match) {
        usedChildIndices.add(match.index);
        const traceId = traceIdToNodeId(match.child.id);
        context.traceNodeMap.set(traceId, nodeId);
        // Also map any nested children
        if (match.child.children && match.child.children.length > 0) {
          mapInlinedChildren(match.child.children, nodeId, context.traceNodeMap);
        }
      }

      cells.push({
        type: 'inline',
        label: formatOperandLabel(operand as JsonLogicValue),
        icon: cellIcon,
        index: idx,
      });
    } else {
      // Complex expression - find matching child by expression content
      const match = findMatchingChild(operand as JsonLogicValue, children, usedChildIndices);
      let branchId: string;

      if (match) {
        usedChildIndices.add(match.index);
        branchId = processExpressionNode(match.child, context, {
          parentId: nodeId,
          argIndex: idx,
        });
      } else {
        // Fallback: create appropriate node based on value type
        branchId = `${nodeId}-arg-${idx}`;
        createFallbackNode(branchId, operand as JsonLogicValue, context, {
          parentId: nodeId,
          argIndex: idx,
        });
      }

      const summary = generateArgSummary(operand as JsonLogicValue);
      summary.label = generateExpressionText(operand as JsonLogicValue, TRUNCATION_LIMITS.expressionText);

      cells.push({
        type: 'branch',
        icon: cellIcon,
        branchId,
        index: idx,
        summary,
      });

      context.edges.push(createBranchEdge(nodeId, branchId, branchIndex));
      branchIndex++;
    }
  });

  const expressionText = generateExpressionText(expression);

  const node: LogicNode = {
    id: nodeId,
    type: 'verticalCell',
    position: { x: 0, y: 0 },
    data: {
      type: 'verticalCell',
      operator,
      category: meta.category,
      label: getOperatorTitle(operator),
      icon,
      cells,
      collapsed: false,
      expressionText,
      expression,
      parentId: parentInfo.parentId,
      argIndex: parentInfo.argIndex,
      branchType: parentInfo.branchType,
    } as VerticalCellNodeData,
  };
  context.nodes.push(node);

  // Add edge from parent if exists and not a branch type
  if (parentInfo.parentId && !parentInfo.branchType) {
    context.edges.push(createArgEdge(parentInfo.parentId, nodeId, parentInfo.argIndex ?? 0));
  }
}
