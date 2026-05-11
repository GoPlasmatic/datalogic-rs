import type { JsonLogicValue } from '../../types';
import { TRUNCATION_LIMITS } from '../../constants/formatting';

// Format value for display in literal nodes (recursive for small arrays)
export function formatValue(value: JsonLogicValue): string {
  if (value === null) return 'null';
  if (typeof value === 'boolean') return value ? 'true' : 'false';
  if (typeof value === 'number') return String(value);
  if (typeof value === 'string') return `"${value}"`;
  if (Array.isArray(value)) {
    if (value.length <= 3) {
      return `[${value.map(formatValue).join(', ')}]`;
    }
    return `[${value.length} items]`;
  }
  return JSON.stringify(value);
}

// Format evaluation result value for compact display (with truncation)
export function formatResultValue(value: unknown): string {
  if (value === null) return 'null';
  if (value === undefined) return 'undefined';
  if (typeof value === 'boolean') return value ? 'true' : 'false';
  if (typeof value === 'number') return String(value);
  if (typeof value === 'string') {
    // Truncate long strings
    if (value.length > TRUNCATION_LIMITS.resultBadge) {
      return `"${value.slice(0, TRUNCATION_LIMITS.resultBadge - 3)}..."`;
    }
    return `"${value}"`;
  }
  if (Array.isArray(value)) {
    if (value.length === 0) return '[]';
    return `[${value.length}]`;
  }
  if (typeof value === 'object') {
    const keys = Object.keys(value as object);
    if (keys.length === 0) return '{}';
    return `{${keys.length}}`;
  }
  return String(value);
}

// Check if a value is complex enough to show in a popover
export function isComplexValue(value: unknown): boolean {
  if (value === null || value === undefined) return false;
  if (typeof value === 'boolean' || typeof value === 'number') return false;
  if (typeof value === 'string') return value.length > 50;
  if (Array.isArray(value)) return value.length > 0;
  if (typeof value === 'object') return Object.keys(value as object).length > 0;
  return false;
}
