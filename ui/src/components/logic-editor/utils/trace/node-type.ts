import type { JsonLogicValue } from '../../types';
import { isDataStructure } from '../type-helpers';
import type { NodeType } from './types';

/**
 * Determine what kind of node to create based on expression
 */
export function determineNodeType(expr: JsonLogicValue, preserveStructure: boolean): NodeType {
  // In preserveStructure mode, check for data structures first
  if (preserveStructure && isDataStructure(expr)) {
    return 'structure';
  }

  // Primitives and arrays -> literal
  if (expr === null || typeof expr !== 'object' || Array.isArray(expr)) {
    return 'literal';
  }

  const keys = Object.keys(expr);
  if (keys.length !== 1) return 'literal'; // Invalid JSONLogic, treat as literal

  const operator = keys[0];

  // Variable operators
  if (['var', 'val', 'exists'].includes(operator)) {
    return 'variable';
  }

  // If/else -> special handling
  if (operator === 'if' || operator === '?:') {
    return 'if';
  }

  // Multi-arg operators -> verticalCell
  const operands = (expr as Record<string, unknown>)[operator];
  const args = Array.isArray(operands) ? operands : [operands];
  if (args.length > 1) {
    return 'verticalCell';
  }

  return 'operator';
}
