/**
 * Argument Parser Utilities
 *
 * Provides parsing and formatting utilities for operator arguments.
 * Extracted from ArgumentsSection for reusability and testability.
 */

import type { LogicNode, LiteralNodeData, JsonLogicValue, OperatorNodeData, VerticalCellNodeData } from '../../types';
import type { Operator } from '../../config/operators.types';

/**
 * Represents an argument that may be inline (literal) or a linked node
 */
export interface ArgumentInfo {
  index: number;
  isInline: boolean;
  /** For inline literals */
  value?: JsonLogicValue;
  valueType?: LiteralNodeData['valueType'];
  /** For linked nodes */
  childNode?: LogicNode;
  childId?: string;
}

/**
 * Check if an operator supports variable arguments
 */
export function supportsVariableArgs(opConfig: Operator | undefined): boolean {
  if (!opConfig) return false;
  return (
    opConfig.arity.type === 'nary' ||
    opConfig.arity.type === 'variadic' ||
    opConfig.arity.type === 'chainable'
  );
}

/**
 * Check if an operator has arguments (any non-nullary operator)
 */
export function hasArguments(opConfig: Operator | undefined): boolean {
  if (!opConfig) return false;
  return opConfig.arity.type !== 'nullary';
}

/**
 * Get the operator name from node data
 */
export function getOperatorName(data: LogicNode['data']): string | null {
  if (data.type === 'operator') return (data as OperatorNodeData).operator;
  if (data.type === 'verticalCell') return (data as VerticalCellNodeData).operator;
  return null;
}

/**
 * Check if a value is a simple literal (not an expression)
 */
export function isSimpleLiteral(value: JsonLogicValue): boolean {
  if (value === null) return true;
  if (typeof value !== 'object') return true;
  if (Array.isArray(value)) return true; // Arrays are literals
  // Objects with operator keys are expressions, not literals
  return false;
}

/**
 * Get the value type for a literal
 */
export function getLiteralType(value: JsonLogicValue): LiteralNodeData['valueType'] {
  if (value === null) return 'null';
  if (typeof value === 'string') return 'string';
  if (typeof value === 'number') return 'number';
  if (typeof value === 'boolean') return 'boolean';
  if (Array.isArray(value)) return 'array';
  return 'array'; // Objects as fallback
}

/**
 * Format a node value for display
 */
export function formatNodeValue(node: LogicNode): string {
  const data = node.data;

  switch (data.type) {
    case 'literal': {
      const value = data.value;
      if (value === null) return 'null';
      if (typeof value === 'string') return `"${value}"`;
      if (typeof value === 'boolean') return value ? 'true' : 'false';
      if (Array.isArray(value)) return `[${value.length} items]`;
      if (typeof value === 'object') return `{...}`;
      return String(value);
    }
    case 'variable':
      return data.path ? `${data.operator}("${data.path}")` : data.operator;
    case 'operator':
      return data.label || data.operator;
    case 'verticalCell':
      return data.label || data.operator;
    case 'decision':
      return 'if(...)';
    case 'structure':
      return data.isArray ? '[...]' : '{...}';
    default:
      return '(expression)';
  }
}

/**
 * Format a raw value for display (for inline literals)
 */
export function formatRawValue(value: JsonLogicValue): string {
  if (value === null) return 'null';
  if (typeof value === 'string') return `"${value}"`;
  if (typeof value === 'boolean') return value ? 'true' : 'false';
  if (Array.isArray(value)) return `[${value.length} items]`;
  if (typeof value === 'object') return `{...}`;
  return String(value);
}

/**
 * Extract arguments from operator node data
 */
export function extractOperatorArguments(
  opData: OperatorNodeData,
  childNodeByArgIndex: Map<number, LogicNode>
): ArgumentInfo[] {
  const expr = opData.expression;

  // Extract operands from expression
  if (expr && typeof expr === 'object' && !Array.isArray(expr)) {
    const operator = Object.keys(expr)[0];
    const operands = (expr as Record<string, unknown>)[operator];
    const operandArray: JsonLogicValue[] = Array.isArray(operands)
      ? operands
      : [operands as JsonLogicValue];

    return operandArray.map((operand, index) => {
      // Check if this operand has a corresponding child node by argIndex
      const childNode = childNodeByArgIndex.get(index);

      if (childNode) {
        // Complex expression with child node
        return {
          index,
          isInline: false,
          childNode,
          childId: childNode.id,
        };
      } else if (isSimpleLiteral(operand)) {
        // Inlined literal
        return {
          index,
          isInline: true,
          value: operand,
          valueType: getLiteralType(operand),
        };
      } else {
        // Complex expression but no child node found (shouldn't happen normally)
        return {
          index,
          isInline: true,
          value: operand,
          valueType: 'array',
        };
      }
    });
  }

  // Fallback: use childIds if no expression
  return opData.childIds.map((childId, index) => {
    const childNode = childNodeByArgIndex.get(index);
    return {
      index,
      isInline: false,
      childNode,
      childId,
    };
  });
}

/**
 * Extract arguments from verticalCell node data
 */
export function extractVerticalCellArguments(
  vcData: VerticalCellNodeData,
  childNodeMap: Map<string, LogicNode>
): ArgumentInfo[] {
  return vcData.cells.map((cell) => {
    if (cell.type === 'inline') {
      // Parse the label to get the value
      let value: JsonLogicValue;
      let valueType: LiteralNodeData['valueType'];

      const label = cell.label || '';
      if (label === 'null') {
        value = null;
        valueType = 'null';
      } else if (label === 'true') {
        value = true;
        valueType = 'boolean';
      } else if (label === 'false') {
        value = false;
        valueType = 'boolean';
      } else if (label.startsWith('"') && label.endsWith('"')) {
        value = label.slice(1, -1);
        valueType = 'string';
      } else if (!isNaN(Number(label))) {
        value = Number(label);
        valueType = 'number';
      } else {
        value = label;
        valueType = 'string';
      }

      return {
        index: cell.index,
        isInline: true,
        value,
        valueType,
      };
    } else {
      // Branch cell with child node
      const childId = cell.branchId || cell.conditionBranchId || cell.thenBranchId;
      const childNode = childId ? childNodeMap.get(childId) : undefined;

      return {
        index: cell.index,
        isInline: false,
        childNode,
        childId,
      };
    }
  });
}
