import type { JsonLogicValue } from '../../types';
import type { ConversionContext } from './types';
import { getParentInfo } from './types';
import { createVariableNode, createArgEdge } from '../node-factory';

// Variable operators
const VARIABLE_OPERATORS = ['var', 'val', 'exists'] as const;
type VariableOperator = (typeof VARIABLE_OPERATORS)[number];

// Check if operator is a variable operator
export function isVariableOperator(operator: string): operator is VariableOperator {
  return VARIABLE_OPERATORS.includes(operator as VariableOperator);
}

// Convert a variable operator (var, val, exists) to a variable node
export function convertVariable(
  operator: VariableOperator,
  operands: JsonLogicValue,
  context: ConversionContext
): string {
  let path: string;
  let defaultValue: JsonLogicValue | undefined;

  if (Array.isArray(operands)) {
    path = String(operands[0] ?? '');
    defaultValue = operands[1];
  } else {
    path = String(operands ?? '');
  }

  // Store the original expression for this variable node
  const originalExpr = { [operator]: operands };
  const parentInfo = getParentInfo(context);

  const node = createVariableNode(
    operator,
    path,
    defaultValue,
    originalExpr,
    parentInfo
  );

  context.nodes.push(node);

  // Add edge from parent if exists and not a branch type
  if (parentInfo.parentId && !parentInfo.branchType) {
    const edge = createArgEdge(parentInfo.parentId, node.id, parentInfo.argIndex ?? 0);
    context.edges.push(edge);
  }

  return node.id;
}
