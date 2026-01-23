import type { JsonLogicValue, LogicNode, VerticalCellNodeData, CellData } from '../../types';
import type { ConversionContext, ConverterFn } from './types';
import { getParentInfo } from './types';
import { generateExpressionText } from '../formatting';
import { createArgEdge } from '../node-factory';
import { v4 as uuidv4 } from 'uuid';

// Convert if/else to a single VerticalCellNode with all branches
// Each condition and then value gets its own row for handle clarity
export function convertIfElse(
  ifArgs: JsonLogicValue[],
  context: ConversionContext,
  convertValue: ConverterFn
): string {
  const parentInfo = getParentInfo(context);
  const nodeId = uuidv4();

  // If there's only a single value (no condition), just convert it directly
  if (ifArgs.length === 1) {
    return convertValue(ifArgs[0], {
      nodes: context.nodes,
      edges: context.edges,
      parentId: parentInfo.parentId,
      argIndex: parentInfo.argIndex,
      branchType: parentInfo.branchType,
      preserveStructure: context.preserveStructure,
    });
  }

  const cells: CellData[] = [];
  let cellIndex = 0;
  let branchIndex = 0;

  // Parse the if-else chain
  let idx = 0;
  while (idx < ifArgs.length - 1) {
    const condition = ifArgs[idx];
    const thenValue = ifArgs[idx + 1];
    const isFirst = idx === 0;

    // Convert condition branch
    const conditionBranchId = convertValue(condition, {
      nodes: context.nodes,
      edges: context.edges,
      parentId: nodeId,
      argIndex: idx,
      preserveStructure: context.preserveStructure,
    });

    // Create condition edge
    context.edges.push({
      id: `${nodeId}-cond-${conditionBranchId}`,
      source: nodeId,
      target: conditionBranchId,
      sourceHandle: `branch-${branchIndex}`,
      targetHandle: 'left',
    });

    // Create cell for condition (If or Else If)
    const conditionText = generateExpressionText(condition, 40);
    cells.push({
      type: 'branch',
      icon: 'diamond',
      rowLabel: isFirst ? 'If' : 'Else If',
      label: conditionText,
      branchId: conditionBranchId,
      index: cellIndex,
    });
    cellIndex++;
    branchIndex++;

    // Convert then branch
    const thenBranchId = convertValue(thenValue, {
      nodes: context.nodes,
      edges: context.edges,
      parentId: nodeId,
      argIndex: idx + 1,
      branchType: 'yes',
      preserveStructure: context.preserveStructure,
    });

    // Create then edge
    context.edges.push({
      id: `${nodeId}-then-${thenBranchId}`,
      source: nodeId,
      target: thenBranchId,
      sourceHandle: `branch-${branchIndex}`,
      targetHandle: 'left',
    });

    // Create cell for then value
    const thenText = generateExpressionText(thenValue, 40);
    cells.push({
      type: 'branch',
      icon: 'check',
      rowLabel: 'Then',
      label: thenText,
      branchId: thenBranchId,
      index: cellIndex,
    });
    cellIndex++;
    branchIndex++;

    idx += 2;
  }

  // Handle final else (if exists)
  const hasFinalElse = ifArgs.length % 2 === 1;
  if (hasFinalElse) {
    const elseValue = ifArgs[ifArgs.length - 1];

    // Convert else branch
    const elseBranchId = convertValue(elseValue, {
      nodes: context.nodes,
      edges: context.edges,
      parentId: nodeId,
      argIndex: ifArgs.length - 1,
      branchType: 'no',
      preserveStructure: context.preserveStructure,
    });

    // Create else edge
    context.edges.push({
      id: `${nodeId}-else-${elseBranchId}`,
      source: nodeId,
      target: elseBranchId,
      sourceHandle: `branch-${branchIndex}`,
      targetHandle: 'left',
    });

    const elseText = generateExpressionText(elseValue, 40);

    cells.push({
      type: 'branch',
      icon: 'x',
      rowLabel: 'Else',
      label: elseText,
      branchId: elseBranchId,
      index: cellIndex,
    });
  }

  // Generate expression text for the entire if/else
  const originalExpr = { if: ifArgs };
  const expressionText = generateExpressionText(originalExpr);

  // Create the VerticalCellNode
  const ifElseNode: LogicNode = {
    id: nodeId,
    type: 'verticalCell',
    position: { x: 0, y: 0 },
    data: {
      type: 'verticalCell',
      operator: 'if',
      category: 'control',
      label: 'If / Then / Else',
      icon: 'diamond',
      cells,
      collapsed: false,
      expressionText,
      collapsedCellIndices: [],
      parentId: parentInfo.parentId,
      argIndex: parentInfo.argIndex,
      branchType: parentInfo.branchType,
      expression: originalExpr,
    } as VerticalCellNodeData,
  };

  context.nodes.push(ifElseNode);

  // Add edge from parent if exists and not a branch connection
  if (parentInfo.parentId && !parentInfo.branchType) {
    context.edges.push(
      createArgEdge(parentInfo.parentId, nodeId, parentInfo.argIndex ?? 0)
    );
  }

  return nodeId;
}
