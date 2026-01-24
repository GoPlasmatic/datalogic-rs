import type { JsonLogicValue, ArgSummary } from '../../types';
import { getOperatorMeta, getOperatorTitle, TRUNCATION_LIMITS } from '../../constants';
import { isPlainObject, looksLikeDate } from '../type-helpers';
import { TYPE_ICONS, VARIABLE_ICONS } from '../icons';

// Generate argument summary for collapsed view
export function generateArgSummary(value: JsonLogicValue): ArgSummary {
  // Handle null
  if (value === null) {
    return { icon: TYPE_ICONS.null, label: 'null', valueType: 'null' };
  }

  // Handle primitives
  if (typeof value === 'boolean') {
    return {
      icon: value ? TYPE_ICONS.boolean_true : TYPE_ICONS.boolean_false,
      label: String(value),
      valueType: 'boolean',
    };
  }

  if (typeof value === 'number') {
    return { icon: TYPE_ICONS.number, label: String(value), valueType: 'number' };
  }

  if (typeof value === 'string') {
    if (looksLikeDate(value)) {
      return { icon: TYPE_ICONS.date, label: value, valueType: 'date' };
    }
    // Truncate long strings
    const displayStr = value.length > TRUNCATION_LIMITS.shortLabel ? value.slice(0, TRUNCATION_LIMITS.shortLabel - 3) + '...' : value;
    return { icon: TYPE_ICONS.string, label: `"${displayStr}"`, valueType: 'string' };
  }

  // Handle arrays
  if (Array.isArray(value)) {
    if (value.length === 0) {
      return { icon: TYPE_ICONS.array, label: '[]', valueType: 'array' };
    }
    return { icon: TYPE_ICONS.array, label: `[${value.length} items]`, valueType: 'array' };
  }

  // Handle objects (expressions)
  if (isPlainObject(value)) {
    const keys = Object.keys(value);
    if (keys.length === 1) {
      const op = keys[0];

      // Variable access
      if (op === 'var') {
        const path = value[op];
        const pathStr = Array.isArray(path) ? String(path[0] ?? '') : String(path ?? '');
        return { icon: VARIABLE_ICONS.var, label: pathStr || 'var', valueType: 'expression' };
      }

      if (op === 'val') {
        const path = value[op];
        const pathStr = Array.isArray(path) ? String(path[0] ?? '') : String(path ?? '');
        return { icon: VARIABLE_ICONS.val, label: `val(${pathStr})`, valueType: 'expression' };
      }

      if (op === 'exists') {
        return { icon: VARIABLE_ICONS.exists, label: `exists(${value[op]})`, valueType: 'expression' };
      }

      // Other operators - show as expression
      const operands = value[op];
      const argCount = Array.isArray(operands) ? operands.length : 1;
      const meta = getOperatorMeta(op);
      return {
        icon: TYPE_ICONS.expression,
        label: `${getOperatorTitle(op)} (${argCount} arg${argCount !== 1 ? 's' : ''})`,
        valueType: 'expression',
      };
    }
  }

  return { icon: TYPE_ICONS.expression, label: '...', valueType: 'expression' };
}

// Format a single operand for display
export function formatOperandLabel(operand: JsonLogicValue): string {
  if (operand === null) return 'null';
  if (typeof operand === 'boolean') return String(operand);
  if (typeof operand === 'number') return String(operand);
  if (typeof operand === 'string') return `"${operand}"`;

  if (Array.isArray(operand)) {
    if (operand.length === 0) return '[]';
    return '[...]';
  }

  if (isPlainObject(operand)) {
    const keys = Object.keys(operand);
    if (keys.length === 1) {
      const op = keys[0];
      if (op === 'var') {
        const path = operand[op];
        if (Array.isArray(path)) {
          return typeof path[0] === 'string' ? path[0] : String(path[0]);
        }
        return typeof path === 'string' ? path : String(path);
      }
      if (op === 'val') {
        const path = operand[op];
        if (Array.isArray(path)) {
          return path.length === 0 ? 'val()' : `val(${path[0]})`;
        }
        return typeof path === 'string' ? `val(${path})` : 'val()';
      }
      if (op === 'exists') {
        const path = operand[op];
        return `exists(${path})`;
      }
    }
  }

  return '...';
}
