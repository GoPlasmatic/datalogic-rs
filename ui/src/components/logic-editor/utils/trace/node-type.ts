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

  // All operators (including var, val, exists, if, multi-arg) -> 'operator'
  return 'operator';
}
