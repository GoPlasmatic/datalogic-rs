import type { JsonLogicValue } from '../../types';
import type { ExpressionNode } from '../../types/trace';
import type { ChildMatch } from './types';

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
  for (let i = 0; i < children.length; i++) {
    if (usedIndices.has(i)) continue;
    try {
      const childExpr = JSON.parse(children[i].expression);
      if (deepEqual(operand, childExpr)) {
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
