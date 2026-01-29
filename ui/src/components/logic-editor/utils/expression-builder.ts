/**
 * Expression Builder Utilities
 *
 * Functions for rebuilding operator expressions from child nodes.
 * This is the single source of truth for expression generation.
 */

import type {
  LogicNode,
  JsonLogicValue,
  LiteralNodeData,
  VariableNodeData,
  OperatorNodeData,
} from '../types';

/**
 * Get the expression value for a node.
 * Returns the node's expression if available, otherwise constructs it from node data.
 */
export function getNodeExpressionValue(node: LogicNode): JsonLogicValue {
  const { data } = node;

  switch (data.type) {
    case 'literal': {
      const litData = data as LiteralNodeData;
      return litData.value as JsonLogicValue;
    }
    case 'variable': {
      const varData = data as VariableNodeData;
      return varData.expression ?? { var: varData.path };
    }
    case 'operator': {
      const opData = data as OperatorNodeData;
      return opData.expression ?? { [opData.operator]: [] };
    }
    case 'verticalCell':
    case 'decision':
    case 'structure':
      // These complex node types should always have an expression set
      return data.expression ?? null;
    default:
      return null;
  }
}

/**
 * Rebuild an operator node's expression from its children.
 * This is the single source of truth for expression generation.
 *
 * @param operator The operator name (e.g., '+', 'and', 'if')
 * @param childNodes The child nodes that form the operands
 * @returns The rebuilt expression object
 */
export function rebuildOperatorExpression(
  operator: string,
  childNodes: LogicNode[]
): JsonLogicValue {
  // Sort children by argIndex to ensure correct order
  const sortedChildren = [...childNodes].sort(
    (a, b) => (a.data.argIndex ?? 0) - (b.data.argIndex ?? 0)
  );

  // Build operands array from child expressions
  const operands: JsonLogicValue[] = sortedChildren.map((child) => {
    return getNodeExpressionValue(child);
  });

  return { [operator]: operands };
}
