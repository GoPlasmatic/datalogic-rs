import type { JsonLogicValue } from '../../types';
import type { ExpressionNode } from '../../types/trace';
import type { ChildMatch } from './types';

/**
 * Find the matching child node for an operand by comparing expressions
 */
export function findMatchingChild(
  operand: JsonLogicValue,
  children: ExpressionNode[],
  usedIndices: Set<number>
): ChildMatch | null {
  const operandStr = JSON.stringify(operand);

  for (let i = 0; i < children.length; i++) {
    if (usedIndices.has(i)) continue;
    // Normalize child expression by parsing and re-stringifying to ensure consistent format
    try {
      const childExprStr = JSON.stringify(JSON.parse(children[i].expression));
      if (childExprStr === operandStr) {
        return { child: children[i], index: i };
      }
    } catch (e) {
      // If parsing fails, try direct comparison
      if (process.env.NODE_ENV === 'development') {
        console.warn('[trace-to-nodes] Failed to parse child expression, using direct comparison:', e);
      }
      if (children[i].expression === operandStr) {
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
