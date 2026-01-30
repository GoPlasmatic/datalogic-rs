import { v4 as uuidv4 } from 'uuid';
import type { JsonLogicValue, LogicNode, OperatorNodeData, CellData } from '../../types';
import type { ConversionContext, ConverterFn } from './types';
import { getParentInfo } from './types';
import { createArgEdge, createBranchEdge, buildVariableCells } from '../node-factory';
import { getOperator } from '../../config/operators';
import { type IconName } from '../icons';
import { getCategoryIcon } from '../../config/categories';
import { generateExpressionText, generateArgSummary } from '../formatting';
import { isSimpleOperand } from '../type-helpers';
import { TRUNCATION_LIMITS } from '../../constants';

// Variable operators
const VARIABLE_OPERATORS = ['var', 'val', 'exists'] as const;
type VariableOperator = (typeof VARIABLE_OPERATORS)[number];

// Check if operator is a variable operator
export function isVariableOperator(operator: string): operator is VariableOperator {
  return VARIABLE_OPERATORS.includes(operator as VariableOperator);
}

// Convert a variable operator (var, val, exists) to a unified operator node with cells
export function convertVariable(
  operator: VariableOperator,
  operands: JsonLogicValue,
  context: ConversionContext,
  convertValue: ConverterFn
): string {
  const nodeId = uuidv4();
  const op = getOperator(operator);
  const category = op?.category ?? 'variable';
  const icon: IconName = getCategoryIcon(category) as IconName;
  let branchIndex = 0;

  // Parse operands to extract path, default value, scope, and path components
  let path = '';
  let defaultValue: JsonLogicValue | undefined;
  let scopeJump: number | undefined;
  let pathComponents: string[] | undefined;

  if (operator === 'var') {
    if (Array.isArray(operands)) {
      path = String(operands[0] ?? '');
      defaultValue = operands[1];
    } else {
      path = String(operands ?? '');
    }
  } else if (operator === 'val') {
    if (Array.isArray(operands)) {
      const [scopeArray, ...pathParts] = operands;
      if (Array.isArray(scopeArray) && scopeArray.length > 0) {
        const scopeValue = scopeArray[0];
        scopeJump = typeof scopeValue === 'number' ? Math.abs(scopeValue) : 0;
      } else {
        scopeJump = 0;
      }
      pathComponents = pathParts.map(p => String(p));
    } else {
      scopeJump = 0;
      pathComponents = [String(operands ?? '')];
    }
  } else if (operator === 'exists') {
    if (Array.isArray(operands)) {
      path = operands.map(p => String(p)).join('.');
    } else {
      path = String(operands ?? '');
    }
  }

  // Build base cells using shared helper
  // For var with complex default, pass undefined so buildVariableCells skips the inline default
  const hasComplexDefault = operator === 'var' && defaultValue !== undefined && !isSimpleOperand(defaultValue);
  const cells: CellData[] = buildVariableCells({
    operator,
    path,
    defaultValue: hasComplexDefault ? undefined : defaultValue,
    scopeJump,
    pathComponents,
  });

  // Handle complex default value as a branch (converter-specific)
  if (hasComplexDefault && defaultValue !== undefined) {
    const branchId = convertValue(defaultValue, {
      nodes: context.nodes,
      edges: context.edges,
      parentId: nodeId,
      argIndex: 1,
      preserveStructure: context.preserveStructure,
    });
    const summary = generateArgSummary(defaultValue);
    summary.label = generateExpressionText(defaultValue, TRUNCATION_LIMITS.expressionText);
    cells.push({
      type: 'branch',
      rowLabel: 'Default',
      icon: 'hash',
      branchId,
      index: 1,
      summary,
    });
    context.edges.push(createBranchEdge(nodeId, branchId, branchIndex));
    branchIndex++;
  }

  const originalExpr = { [operator]: operands };
  const expressionText = generateExpressionText(originalExpr);
  const parentInfo = getParentInfo(context);

  const variableNode: LogicNode = {
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
  context.nodes.push(variableNode);

  // Add edge from parent if exists and not a branch type
  if (parentInfo.parentId && !parentInfo.branchType) {
    const edge = createArgEdge(parentInfo.parentId, nodeId, parentInfo.argIndex ?? 0);
    context.edges.push(edge);
  }

  return nodeId;
}

