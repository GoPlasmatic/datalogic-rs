import { v4 as uuidv4 } from 'uuid';
import type { JsonLogicValue, CellData, LogicNode, OperatorNodeData } from '../../types';
import type { ConversionContext, ConverterFn } from './types';
import { getParentInfo } from './types';
import { TRUNCATION_LIMITS } from '../../constants';
import { getOperator } from '../../config/operators';
import { ITERATOR_ARG_ICONS, getOperandTypeIcon, CONTROL_ICONS, type IconName } from '../icons';
import { getCategoryIcon } from '../../config/categories';
import { generateExpressionText, generateArgSummary, formatOperandLabel } from '../formatting';
import { isSimpleOperand } from '../type-helpers';
import { createBranchEdge, createArgEdge } from '../node-factory';
import { inlineVarIndices } from '../inline-vars';

// Unary operators for special handling
const UNARY_OPERATORS = ['!', '!!'];

// Convert any operator to a unified operator node with cells
export function convertOperator(
  operator: string,
  operandArray: JsonLogicValue[],
  context: ConversionContext,
  convertValue: ConverterFn
): string {
  const nodeId = uuidv4();
  const op = getOperator(operator);
  const category = op?.category ?? 'utility';
  const cells: CellData[] = [];

  // Determine icon based on operator type
  let icon: IconName = getCategoryIcon(category) as IconName;
  if (operator === 'or') icon = CONTROL_ICONS.orOperator;

  // Get iterator argument icons if applicable
  const iteratorIcons = ITERATOR_ARG_ICONS[operator];

  // Signal Board: static var/val reads collapse into inline pills instead of their
  // own child nodes (capped per operator — see inline-vars). AND / OR / NOT are the
  // exception: they always wire every operand out so the gate silhouette renders.
  const forceChildren = category === 'logical';
  const inlineVarIdx = forceChildren ? new Set<number>() : inlineVarIndices(operandArray);

  operandArray.forEach((operand, idx) => {
    // Determine the cell icon - use type-based icon, but override with iterator icons for iterators
    const typeIcon = getOperandTypeIcon(operand);
    const cellIcon = iteratorIcons ? iteratorIcons[idx] || typeIcon : typeIcon;

    // Display inline when it's a simple literal or a collapsible static var read.
    if (!forceChildren && (isSimpleOperand(operand) || inlineVarIdx.has(idx))) {
      cells.push({
        type: 'inline',
        label: formatOperandLabel(operand),
        icon: cellIcon,
        index: idx,
      });
    } else {
      // Complex expression - create branch with summary and expression text
      const branchId = convertValue(operand, {
        nodes: context.nodes,
        edges: context.edges,
        parentId: nodeId,
        argIndex: idx,
        branchType: 'branch',
        templating: context.templating,
      });
      const summary = generateArgSummary(operand);
      summary.label = generateExpressionText(operand, TRUNCATION_LIMITS.expressionText);
      cells.push({
        type: 'branch',
        icon: cellIcon,
        branchId,
        index: idx,
        summary,
      });

      // Add edge from this node to the branch (use idx to match cell.index / CellHandles)
      context.edges.push(createBranchEdge(nodeId, branchId, idx));
    }
  });

  // Generate full expression text for the node
  const originalExpr = { [operator]: operandArray };
  const expressionText = generateExpressionText(originalExpr);
  const parentInfo = getParentInfo(context);

  const operatorNode: LogicNode = {
    id: nodeId,
    type: 'operator',
    position: { x: 0, y: 0 },
    data: {
      type: 'operator',
      operator,
      category,
      label: op?.label ?? operator,
      icon,
      cells,
      collapsed: false,
      expressionText,
      expression: originalExpr,
      parentId: parentInfo.parentId,
      argIndex: parentInfo.argIndex,
      branchType: parentInfo.branchType,
    } as OperatorNodeData,
  };
  context.nodes.push(operatorNode);

  // Add edge from parent if exists
  if (parentInfo.parentId && !parentInfo.branchType) {
    context.edges.push(
      createArgEdge(parentInfo.parentId, nodeId, parentInfo.argIndex ?? 0)
    );
  }

  return nodeId;
}

// Check if operator is a unary operator
export function isUnaryOperator(operator: string): boolean {
  return UNARY_OPERATORS.includes(operator);
}
