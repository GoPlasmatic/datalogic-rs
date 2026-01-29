import { v4 as uuidv4 } from 'uuid';
import type { JsonLogicValue, CellData, LogicNode, VerticalCellNodeData, OperatorNodeData } from '../../types';
import type { ConversionContext, ConverterFn } from './types';
import { getParentInfo } from './types';
import { TRUNCATION_LIMITS } from '../../constants';
import { getOperatorMeta, getOperatorTitle } from '../../config/operators';
import { CATEGORY_ICONS, ITERATOR_ARG_ICONS, getOperandTypeIcon, CONTROL_ICONS, type IconName } from '../icons';
import { generateExpressionText, generateArgSummary, formatOperandLabel } from '../formatting';
import { isSimpleOperand } from '../type-helpers';
import { createBranchEdge, createArgEdge } from '../node-factory';

// Unary operators for special handling
const UNARY_OPERATORS = ['!', '!!'];

// Convert to vertical cell node for comparison, logical, and iterator operators
export function convertToVerticalCell(
  operator: string,
  operandArray: JsonLogicValue[],
  context: ConversionContext,
  convertValue: ConverterFn
): string {
  const nodeId = uuidv4();
  const meta = getOperatorMeta(operator);
  const cells: CellData[] = [];
  let branchIndex = 0;

  // Determine icon based on operator type
  let icon: IconName = CATEGORY_ICONS[meta.category] || 'list';
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

  const verticalCellNode: LogicNode = {
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
      expression: originalExpr,
      parentId: parentInfo.parentId,
      argIndex: parentInfo.argIndex,
      branchType: parentInfo.branchType,
    } as VerticalCellNodeData,
  };
  context.nodes.push(verticalCellNode);

  // Add edge from parent if exists
  if (parentInfo.parentId && !parentInfo.branchType) {
    context.edges.push(
      createArgEdge(parentInfo.parentId, nodeId, parentInfo.argIndex ?? 0)
    );
  }

  return nodeId;
}

// Convert unary operator with simple argument (inline display)
export function convertUnaryInline(
  operator: string,
  expressionText: string,
  value: JsonLogicValue,
  context: ConversionContext
): string {
  const nodeId = uuidv4();
  const meta = getOperatorMeta(operator);
  const parentInfo = getParentInfo(context);

  const operatorNode: LogicNode = {
    id: nodeId,
    type: 'operator',
    position: { x: 0, y: 0 },
    data: {
      type: 'operator',
      operator,
      category: meta.category,
      label: getOperatorTitle(operator),
      childIds: [], // No children - inline display
      collapsed: false,
      expressionText,
      expression: value,
      inlineDisplay: expressionText, // Show inline
      parentId: parentInfo.parentId,
      argIndex: parentInfo.argIndex,
      branchType: parentInfo.branchType,
    } as OperatorNodeData,
  };
  context.nodes.push(operatorNode);

  if (parentInfo.parentId && !parentInfo.branchType) {
    context.edges.push(
      createArgEdge(parentInfo.parentId, nodeId, parentInfo.argIndex ?? 0)
    );
  }

  return nodeId;
}

// Convert standard operator with child nodes
export function convertOperatorWithChildren(
  operator: string,
  operandArray: JsonLogicValue[],
  value: JsonLogicValue,
  context: ConversionContext,
  convertValue: ConverterFn
): string {
  const nodeId = uuidv4();
  const meta = getOperatorMeta(operator);
  const expressionText = generateExpressionText(value);
  const childIds: string[] = [];

  // Process each operand recursively
  operandArray.forEach((operand, idx) => {
    const childId = convertValue(operand, {
      nodes: context.nodes,
      edges: context.edges,
      parentId: nodeId,
      argIndex: idx,
      preserveStructure: context.preserveStructure,
    });
    childIds.push(childId);
  });

  const parentInfo = getParentInfo(context);

  const operatorNode: LogicNode = {
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
      expression: value,
      parentId: parentInfo.parentId,
      argIndex: parentInfo.argIndex,
      branchType: parentInfo.branchType,
    } as OperatorNodeData,
  };
  context.nodes.push(operatorNode);

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
