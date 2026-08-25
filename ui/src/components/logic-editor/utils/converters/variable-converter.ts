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
import { convertOperator } from './operator-converter';
import {
  isVariableOperatorName,
  parseVarOperand,
  parseValOperand,
  parseExistsOperand,
  type VariableOperator,
} from './variable-cells';

// Check if operator is a variable operator
export function isVariableOperator(operator: string): operator is VariableOperator {
  return isVariableOperatorName(operator);
}

// Convert a variable operator (var, val, exists) to a unified operator node with cells.
// A computed path (an expression where a path segment is expected) falls back to
// the generic operator converter so it is wired as a child and round-trips intact.
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

  // Parse operands to extract path, default value, scope, and path components
  let path: string | number = '';
  let defaultValue: JsonLogicValue | undefined;
  let scopeJump: number | undefined;
  let pathComponents: JsonLogicValue[] | undefined;

  if (operator === 'var') {
    const parsed = parseVarOperand(operands);
    if (!parsed.isStatic) return convertDynamic(operator, operands, context, convertValue);
    path = parsed.path;
    defaultValue = parsed.defaultValue;
  } else if (operator === 'val') {
    const parsed = parseValOperand(operands);
    if (!parsed.isStatic) return convertDynamic(operator, operands, context, convertValue);
    scopeJump = parsed.scope;
    pathComponents = parsed.components;
  } else if (operator === 'exists') {
    const parsed = parseExistsOperand(operands);
    if (!parsed.isStatic) return convertDynamic(operator, operands, context, convertValue);
    if (Array.isArray(parsed.path)) {
      pathComponents = parsed.path;
    } else {
      path = parsed.path;
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
      branchType: 'branch',
      templating: context.templating,
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
    // The branch edge uses the cell index so it matches CellHandles / edge-builder.
    context.edges.push(createBranchEdge(nodeId, branchId, 1));
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

// A var/val/exists whose path is computed: wire the operands as ordinary
// arguments. The generic node keeps the raw operand on its expression, so the
// serializer emits the original single-value or array form.
function convertDynamic(
  operator: VariableOperator,
  operands: JsonLogicValue,
  context: ConversionContext,
  convertValue: ConverterFn
): string {
  const operandArray: JsonLogicValue[] = Array.isArray(operands) ? operands : [operands];
  return convertOperator(operator, operandArray, context, convertValue, operands);
}
