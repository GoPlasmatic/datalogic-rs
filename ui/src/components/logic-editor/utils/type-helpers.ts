import type { JsonLogicValue, LiteralNodeData } from '../types';
import { isOperator } from '../constants/operators';

// Check if value is a plain object (not array, not null)
export function isPlainObject(value: unknown): value is Record<string, unknown> {
  return typeof value === 'object' && value !== null && !Array.isArray(value);
}

// Check if value is a JSONLogic expression (single-key object where key is an operator)
export function isJsonLogicExpression(value: unknown): boolean {
  if (!isPlainObject(value)) return false;
  const keys = Object.keys(value);
  return keys.length === 1 && isOperator(keys[0]);
}

// Check if value is a data structure (object/array that may contain JSONLogic)
// Used in preserveStructure mode to detect template objects
export function isDataStructure(value: unknown): boolean {
  // Arrays with content may contain JSONLogic expressions
  if (Array.isArray(value)) return value.length > 0;

  // Plain objects that are NOT JSONLogic expressions (multi-key or unknown single-key)
  if (!isPlainObject(value)) return false;
  const keys = Object.keys(value);

  // Multi-key objects are data structures
  if (keys.length > 1) return true;

  // Single-key objects where the key is NOT an operator are data structures
  return keys.length === 1 && !isOperator(keys[0]);
}

// Get the value type for literals
export function getValueType(value: JsonLogicValue): LiteralNodeData['valueType'] {
  if (value === null) return 'null';
  if (typeof value === 'boolean') return 'boolean';
  if (typeof value === 'number') return 'number';
  if (typeof value === 'string') return 'string';
  if (Array.isArray(value)) return 'array';
  return 'string';
}

// Check if a string looks like a date
export function looksLikeDate(str: string): boolean {
  // Check common date patterns
  const datePatterns = [
    /^\d{4}-\d{2}-\d{2}/, // ISO date
    /^\d{2}\/\d{2}\/\d{4}/, // MM/DD/YYYY
    /^\d{2}-\d{2}-\d{4}/, // DD-MM-YYYY
  ];
  return datePatterns.some(pattern => pattern.test(str));
}

// Check if an operand is "simple" (can be displayed inline)
// Only literals are simple - variable operators (var, val, exists) get their own nodes
// so their debug context/results show correctly
export function isSimpleOperand(operand: JsonLogicValue): boolean {
  // Primitives (literals) are simple
  if (operand === null || typeof operand !== 'object') {
    return true;
  }

  // Arrays are not simple (except empty arrays for display)
  if (Array.isArray(operand)) {
    return operand.length === 0;
  }

  // Objects (including var/val/exists operators) are not simple
  // They need their own visual nodes for proper debug highlighting
  return false;
}

// Get CSS class for color-coding values by type in debug displays
export function getValueColorClass(value: unknown): string {
  if (value === null) return 'debug-value-null';
  if (value === undefined) return 'debug-value-undefined';
  if (typeof value === 'boolean') {
    return value ? 'debug-value-boolean-true' : 'debug-value-boolean-false';
  }
  if (typeof value === 'number') return 'debug-value-number';
  if (typeof value === 'string') return 'debug-value-string';
  if (Array.isArray(value)) return 'debug-value-array';
  if (typeof value === 'object') return 'debug-value-object';
  return '';
}
