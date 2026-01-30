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
  let branchIndex = 0;

  // Determine icon based on operator type
  let icon: IconName = getCategoryIcon(category) as IconName;
  if (operator === 'or') icon = CONTROL_ICONS.orOperator;

  // Get iterator argument icons if applicable
  const iteratorIcons = ITERATOR_ARG_ICONS[operator];

  operandArray.forEach((operand, idx) => {
    // Determine the cell icon - use type-based icon, but override with iterator icons for iterators
    const typeIcon = getOperandTypeIcon(operand);
    const cellIcon = iteratorIcons ? iteratorIcons[idx] || typeIcon : typeIcon;

    // Check if operand can be displayed inline (simple literal or variable)
    if (isSimpleOperand(operand)) {
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
        preserveStructure: context.preserveStructure,
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

      // Add edge from this node to the branch
      context.edges.push(createBranchEdge(nodeId, branchId, branchIndex));
      branchIndex++;
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
