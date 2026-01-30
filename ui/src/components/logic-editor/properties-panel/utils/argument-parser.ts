/**
 * Argument Parser Utilities
 *
 * Provides parsing and formatting utilities for operator arguments.
 * Extracted from ArgumentsSection for reusability and testability.
 */

import type { LogicNode, LiteralNodeData, JsonLogicValue, OperatorNodeData } from '../../types';
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
  /** Row label from cell (e.g., 'If', 'Then', 'Else If', 'Else', 'Path') */
  rowLabel?: string;
  /** Field identifier from editable cell (e.g., 'path', 'scopeLevel') */
  fieldId?: string;
  /** Field input type from editable cell (e.g., 'text', 'number') */
  fieldType?: string;
  /** Placeholder text from editable cell */
  placeholder?: string;
}

/**
 * Check if an operator supports variable arguments
 */
export function supportsVariableArgs(opConfig: Operator | undefined): boolean {
  if (!opConfig) return false;
  return (
    opConfig.arity.type === 'nary' ||
    opConfig.arity.type === 'variadic' ||
    opConfig.arity.type === 'chainable' ||
    opConfig.arity.type === 'special' ||
    opConfig.arity.type === 'range'
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
    case 'operator':
      return data.label || data.operator;
    case 'structure':
      return data.isArray ? '[...]' : '{...}';
    default:
      return '(expression)';
  }
}

/**
 * Extract arguments from operator node data (cells-based)
 */
export function extractArguments(
  opData: OperatorNodeData,
  childNodeMap: Map<string, LogicNode>
): ArgumentInfo[] {
  return opData.cells.map((cell) => {
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
        rowLabel: cell.rowLabel,
      };
    } else if (cell.type === 'editable') {
      // Editable cell (var path, val scope, etc.) — show as inline editable
      const cellValue = cell.value;
      let value: JsonLogicValue;
      let valueType: LiteralNodeData['valueType'];

      if (cellValue === null || cellValue === undefined) {
        value = '';
        valueType = 'string';
      } else if (typeof cellValue === 'number') {
        value = cellValue;
        valueType = 'number';
      } else {
        value = String(cellValue);
        valueType = 'string';
      }

      return {
        index: cell.index,
        isInline: true,
        value,
        valueType,
        rowLabel: cell.rowLabel,
        fieldId: cell.fieldId,
        fieldType: cell.fieldType,
        placeholder: cell.placeholder,
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
        rowLabel: cell.rowLabel,
      };
    }
  });
}
