import type { JsonLogicValue } from '../../types';
import type { ExpressionNode } from '../../types/trace';
import type { ChildMatch } from './types';

/**
 * Normalize a JSONLogic expression so that single-arg operators use unwrapped form.
 * The Rust compiler normalizes {"op": [x]} to {"op": x} for single-arg operators,
 * but the original input may use either form. This ensures consistent comparison.
 */
function normalizeExpression(expr: unknown): unknown {
  if (expr === null || typeof expr !== 'object') return expr;
  if (Array.isArray(expr)) return expr.map(normalizeExpression);

  const obj = expr as Record<string, unknown>;
  const keys = Object.keys(obj);
  if (keys.length !== 1) {
    // Multi-key object — normalize values recursively
    const result: Record<string, unknown> = {};
    for (const k of keys) result[k] = normalizeExpression(obj[k]);
    return result;
  }

  // Single-key object = JSONLogic operator
  const key = keys[0];
  let value = obj[key];

  // Unwrap single-element arrays: {"op": [x]} → {"op": x}
  if (Array.isArray(value) && value.length === 1) {
    value = value[0];
  }

  return { [key]: normalizeExpression(value) };
}

/**
 * Deep equality comparison that ignores object key ordering.
 * Rust's serde_json uses BTreeMap (alphabetical keys) while JS preserves insertion order.
 */
function deepEqual(a: unknown, b: unknown): boolean {
  if (a === b) return true;
  if (a === null || b === null || typeof a !== typeof b) return a === b;
  if (Array.isArray(a)) {
    if (!Array.isArray(b) || a.length !== b.length) return false;
    return a.every((v, i) => deepEqual(v, b[i]));
  }
  if (typeof a === 'object') {
    const aObj = a as Record<string, unknown>;
    const bObj = b as Record<string, unknown>;
    const aKeys = Object.keys(aObj);
    const bKeys = Object.keys(bObj);
    if (aKeys.length !== bKeys.length) return false;
    return aKeys.every(k => k in bObj && deepEqual(aObj[k], bObj[k]));
  }
  return false;
}

/**
 * Find the matching child node for an operand by comparing expressions.
 * Uses deep equality to handle key ordering differences between JS and Rust's BTreeMap.
 */
export function findMatchingChild(
  operand: JsonLogicValue,
  children: ExpressionNode[],
  usedIndices: Set<number>
): ChildMatch | null {
  const normalizedOperand = normalizeExpression(operand);
  for (let i = 0; i < children.length; i++) {
    if (usedIndices.has(i)) continue;
    try {
      const childExpr = JSON.parse(children[i].expression);
      // First try exact match, then try normalized match.
      // Normalization handles the difference between {"op": [x]} and {"op": x}
      // which arises because the Rust compiler unwraps single-element arrays.
      if (deepEqual(operand, childExpr) || deepEqual(normalizedOperand, normalizeExpression(childExpr))) {
        return { child: children[i], index: i };
      }
    } catch {
      // If parsing fails, fall back to string comparison
      if (children[i].expression === JSON.stringify(operand)) {
        return { child: children[i], index: i };
      }
    }
  }
  return null;
}

/**
 * Get the next unused child (for positional matching when exact matching fails)
 */
export function getNextUnusedChild(
  children: ExpressionNode[],
  usedIndices: Set<number>
): ChildMatch | null {
  for (let i = 0; i < children.length; i++) {
    if (!usedIndices.has(i)) {
      return { child: children[i], index: i };
    }
  }
  return null;
}
