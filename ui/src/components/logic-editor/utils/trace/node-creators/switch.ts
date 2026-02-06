import type {
  LogicNode,
  JsonLogicValue,
  CellData,
  OperatorNodeData,
} from '../../../types';
import type { ExpressionNode } from '../../../types/trace';
import type { ParentInfo } from '../../converters/types';
import type { TraceContext } from '../types';
import { generateExpressionText, formatOperandLabel } from '../../formatting';
import { isSimpleOperand } from '../../type-helpers';
import { createBranchEdge, createArgEdge } from '../../node-factory';
import { findMatchingChild, getNextUnusedChild } from '../child-matching';
import { determineNodeType } from '../node-type';
import type { NodeType } from '../types';

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
 * Create a VerticalCellNode for switch/match expressions from trace data.
 *
 * JSON structure: {"switch": [discriminant, [[case1, result1], ...], default]}
 *
 * Visual layout mirrors if/else but with case/then pairs:
 *   Match   → discriminant
 *   Case    → case value (inline or branch)
 *   Then    → result
 *   ...
 *   Default → default value
 */
export function createSwitchNodeFromTrace(
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
  const switchArgs = obj[operator] as JsonLogicValue[];
  const usedChildIndices = new Set<number>();

  const cells: CellData[] = [];
  let cellIndex = 0;
  let branchIndex = 0;

  // Helper to process a branch value from trace
  function processBranch(
    value: JsonLogicValue,
    argIndex: number,
    branchType?: 'yes' | 'no'
  ): string {
    let branchId: string;
    const match = findMatchingChild(value, children, usedChildIndices);
    if (match) {
      usedChildIndices.add(match.index);
      branchId = processExpressionNode(match.child, context, {
        parentId: nodeId,
        argIndex,
        branchType,
      });
    } else {
      const nodeType: NodeType = determineNodeType(value, context.preserveStructure);
      const nextUnused = nodeType !== 'literal' ? getNextUnusedChild(children, usedChildIndices) : null;
      if (nextUnused) {
        usedChildIndices.add(nextUnused.index);
        branchId = processExpressionNode(nextUnused.child, context, {
          parentId: nodeId,
          argIndex,
          branchType,
        }, value);
      } else {
        branchId = `${nodeId}-arg-${argIndex}`;
        createFallbackNode(branchId, value, context, {
          parentId: nodeId,
          argIndex,
          branchType,
        });
      }
    }
    return branchId;
  }

  // args[0] = discriminant
  if (switchArgs.length >= 1) {
    const discriminant = switchArgs[0];

    if (isSimpleOperand(discriminant)) {
      cells.push({
        type: 'inline',
        icon: 'diamond',
        rowLabel: 'Match',
        label: formatOperandLabel(discriminant),
        index: cellIndex,
      });
    } else {
      const discBranchId = processBranch(discriminant, 0);
      context.edges.push(createBranchEdge(nodeId, discBranchId, branchIndex));

      cells.push({
        type: 'branch',
        icon: 'diamond',
        rowLabel: 'Match',
        label: generateExpressionText(discriminant, 40),
        branchId: discBranchId,
        index: cellIndex,
      });
      branchIndex++;
    }
    cellIndex++;
  }

  // args[1] = cases array: [[case_val, result], ...]
  if (switchArgs.length >= 2) {
    const cases = switchArgs[1];
    const casePairs: JsonLogicValue[][] = Array.isArray(cases)
      ? (cases as JsonLogicValue[][])
      : [];

    for (let i = 0; i < casePairs.length; i++) {
      const pair = casePairs[i];
      if (!Array.isArray(pair) || pair.length < 2) continue;

      const caseValue = pair[0];
      const resultValue = pair[1];

      // Case value row
      if (isSimpleOperand(caseValue)) {
        cells.push({
          type: 'inline',
          icon: 'tag',
          rowLabel: 'Case',
          label: formatOperandLabel(caseValue),
          index: cellIndex,
        });
      } else {
        const caseBranchId = processBranch(caseValue, cellIndex);
        context.edges.push(createBranchEdge(nodeId, caseBranchId, branchIndex));

        cells.push({
          type: 'branch',
          icon: 'tag',
          rowLabel: 'Case',
          label: generateExpressionText(caseValue, 40),
          branchId: caseBranchId,
          index: cellIndex,
        });
        branchIndex++;
      }
      cellIndex++;

      // Result value row (Then)
      if (isSimpleOperand(resultValue)) {
        cells.push({
          type: 'inline',
          icon: 'check',
          rowLabel: 'Then',
          label: formatOperandLabel(resultValue),
          index: cellIndex,
        });
      } else {
        const resultBranchId = processBranch(resultValue, cellIndex, 'yes');
        context.edges.push(createBranchEdge(nodeId, resultBranchId, branchIndex));

        cells.push({
          type: 'branch',
          icon: 'check',
          rowLabel: 'Then',
          label: generateExpressionText(resultValue, 40),
          branchId: resultBranchId,
          index: cellIndex,
        });
        branchIndex++;
      }
      cellIndex++;
    }
  }

  // args[2] = default (optional)
  if (switchArgs.length >= 3) {
    const defaultValue = switchArgs[2];

    if (isSimpleOperand(defaultValue)) {
      cells.push({
        type: 'inline',
        icon: 'x',
        rowLabel: 'Default',
        label: formatOperandLabel(defaultValue),
        index: cellIndex,
      });
    } else {
      const defaultBranchId = processBranch(defaultValue, cellIndex, 'no');
      context.edges.push(createBranchEdge(nodeId, defaultBranchId, branchIndex));

      cells.push({
        type: 'branch',
        icon: 'x',
        rowLabel: 'Default',
        label: generateExpressionText(defaultValue, 40),
        branchId: defaultBranchId,
        index: cellIndex,
      });
    }
  }

  // Generate expression text
  const expressionText = generateExpressionText(expression);
  const label = operator === 'match' ? 'Match / Case' : 'Switch / Case';

  const switchNode: LogicNode = {
    id: nodeId,
    type: 'operator',
    position: { x: 0, y: 0 },
    data: {
      type: 'operator',
      operator,
      category: 'control',
      label,
      icon: 'diamond',
      cells,
      collapsed: false,
      expressionText,
      parentId: parentInfo.parentId,
      argIndex: parentInfo.argIndex,
      branchType: parentInfo.branchType,
      expression,
    } as OperatorNodeData,
  };
  context.nodes.push(switchNode);

  // Add edge from parent if exists and not a branch type
  if (parentInfo.parentId && !parentInfo.branchType) {
    context.edges.push(createArgEdge(parentInfo.parentId, nodeId, parentInfo.argIndex ?? 0));
  }
}
