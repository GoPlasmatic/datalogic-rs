import type { JsonLogicValue, ArgSummary } from '../../types';
import { TRUNCATION_LIMITS } from '../../constants';
import { getOperator } from '../../config/operators';
import { isPlainObject, looksLikeDate } from '../type-helpers';
import { TYPE_ICONS, VARIABLE_ICONS } from '../icons';

// Whole-current-context read: {var:""} / {val:""} / {val:[[0]]}.
export const CONTEXT_LABEL = 'context';

/**
 * Compact display for a var/val/exists data read, handling the two tricky cases:
 *   • empty path — the whole current context (`{var:""}` → "context").
 *   • val scope-jump — `{val:[[-1],"threshold"]}` → "↑threshold" (n up-arrows).
 */
export function formatVarValPath(op: 'var' | 'val' | 'exists', raw: JsonLogicValue): string {
  if (op === 'val') {
    let up = 0;
    let parts: JsonLogicValue[];
    if (Array.isArray(raw)) {
      const [scope, ...rest] = raw;
      if (Array.isArray(scope)) {
        const n = scope[0];
        up = typeof n === 'number' ? Math.abs(n) : 0;
        parts = rest;
      } else {
        parts = raw; // no scope array — the whole thing is the path
      }
    } else {
      parts = [raw];
    }
    const path = parts.map((p) => String(p ?? '')).filter((s) => s !== '').join('.');
    const prefix = up > 0 ? (up === 1 ? '↑' : `↑${up} `) : '';
    return prefix + (path || CONTEXT_LABEL);
  }
  // var / exists
  let path: string;
  if (Array.isArray(raw)) {
    path = op === 'exists' ? raw.map((p) => String(p ?? '')).join('.') : String(raw[0] ?? '');
  } else {
    path = String(raw ?? '');
  }
  if (path === '') return CONTEXT_LABEL;
  return op === 'exists' ? `exists(${path})` : path;
}

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

      // Variable access — var / val / exists
      if (op === 'var' || op === 'val' || op === 'exists') {
        const icon =
          op === 'var' ? VARIABLE_ICONS.var : op === 'val' ? VARIABLE_ICONS.val : VARIABLE_ICONS.exists;
        return { icon, label: formatVarValPath(op, value[op]), valueType: 'expression' };
      }

      // Other operators - show as expression
      const operands = value[op];
      const argCount = Array.isArray(operands) ? operands.length : 1;
      return {
        icon: TYPE_ICONS.expression,
        label: `${getOperator(op)?.label ?? op} (${argCount} arg${argCount !== 1 ? 's' : ''})`,
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
      if (op === 'var' || op === 'val' || op === 'exists') {
        return formatVarValPath(op, operand[op]);
      }
    }
  }

  return '...';
}
