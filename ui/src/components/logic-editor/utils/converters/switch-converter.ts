import type { JsonLogicValue, LogicNode, OperatorNodeData, CellData } from '../../types';
import type { ConversionContext, ConverterFn } from './types';
import { getParentInfo } from './types';
import { generateExpressionText } from '../formatting';
import { createBranchEdge, createArgEdge } from '../node-factory';
import { isSimpleOperand } from '../type-helpers';
import { formatOperandLabel } from '../formatting';
import { v4 as uuidv4 } from 'uuid';

/**
 * Convert switch/match to a single VerticalCellNode with all branches.
 *
 * JSON structure: {"switch": [discriminant, [[case1, result1], [case2, result2], ...], default]}
 *
 * Visual layout:
 *   Match  → discriminant expression
 *   Case   → case value (inline or branch)
 *   Then   → result expression
 *   Case   → case value
 *   Then   → result expression
 *   ...
 *   Default → default expression
 */
export function convertSwitch(
  operator: string,
  switchArgs: JsonLogicValue[],
  context: ConversionContext,
  convertValue: ConverterFn
): string {
  const parentInfo = getParentInfo(context);
  const nodeId = uuidv4();

  const cells: CellData[] = [];
  let cellIndex = 0;
  let branchIndex = 0;

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
      const discBranchId = convertValue(discriminant, {
        nodes: context.nodes,
        edges: context.edges,
        parentId: nodeId,
        argIndex: 0,
        preserveStructure: context.preserveStructure,
      });

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
        const caseBranchId = convertValue(caseValue, {
          nodes: context.nodes,
          edges: context.edges,
          parentId: nodeId,
          argIndex: cellIndex,
          preserveStructure: context.preserveStructure,
        });

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
        const resultBranchId = convertValue(resultValue, {
          nodes: context.nodes,
          edges: context.edges,
          parentId: nodeId,
          argIndex: cellIndex,
          branchType: 'yes',
          preserveStructure: context.preserveStructure,
        });

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
      const defaultBranchId = convertValue(defaultValue, {
        nodes: context.nodes,
        edges: context.edges,
        parentId: nodeId,
        argIndex: cellIndex,
        branchType: 'no',
        preserveStructure: context.preserveStructure,
      });

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

  // Generate expression text for the entire switch
  const originalExpr = { [operator]: switchArgs };
  const expressionText = generateExpressionText(originalExpr);

  const label = operator === 'match' ? 'Match / Case' : 'Switch / Case';

  // Create the unified operator node
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
      expression: originalExpr,
    } as OperatorNodeData,
  };

  context.nodes.push(switchNode);

  // Add edge from parent if exists and not a branch connection
  if (parentInfo.parentId && !parentInfo.branchType) {
    context.edges.push(
      createArgEdge(parentInfo.parentId, nodeId, parentInfo.argIndex ?? 0)
    );
  }

  return nodeId;
}
