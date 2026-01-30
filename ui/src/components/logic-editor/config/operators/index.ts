/**
 * Operator Configuration Index
 *
 * This file exports all operator configurations and provides helper functions
 * for looking up operators by name, category, or other criteria.
 */

import type { Operator, OperatorCategory } from '../operators.types';
import { variableOperators } from './variable';
import { comparisonOperators } from './comparison';
import { logicalOperators } from './logical';
import { arithmeticOperators } from './arithmetic';
import { controlOperators } from './control';
import { stringOperators } from './string';
import { arrayOperators } from './array';
import { datetimeOperators } from './datetime';
import { validationOperators } from './validation';
import { errorOperators } from './error';
import { utilityOperators } from './utility';

// Re-export individual category modules
export {
  variableOperators,
  comparisonOperators,
  logicalOperators,
  arithmeticOperators,
  controlOperators,
  stringOperators,
  arrayOperators,
  datetimeOperators,
  validationOperators,
  errorOperators,
  utilityOperators,
};

/**
 * Combined map of all operators by name
 */
export const operators: Record<string, Operator> = {
  ...variableOperators,
  ...comparisonOperators,
  ...logicalOperators,
  ...arithmeticOperators,
  ...controlOperators,
  ...stringOperators,
  ...arrayOperators,
  ...datetimeOperators,
  ...validationOperators,
  ...errorOperators,
  ...utilityOperators,
};

/**
 * Get an operator by its name
 * @param name - The operator name (e.g., "var", "+", "if")
 * @returns The operator configuration, or undefined if not found
 */
export function getOperator(name: string): Operator | undefined {
  return operators[name];
}

/**
 * Get all operators in a specific category
 * @param category - The category to filter by
 * @returns Array of operators in the category
 */
export function getOperatorsByCategory(category: OperatorCategory): Operator[] {
  return Object.values(operators).filter((op) => op.category === category);
}

/**
 * Check if a string is a valid operator name
 * @param name - String to check
 * @returns true if the name is a valid operator
 */
export function isOperator(name: string): boolean {
  return name in operators;
}

/**
 * Get operators grouped by category
 * @returns Map of category to operators
 */
export function getOperatorsGroupedByCategory(): Map<OperatorCategory, Operator[]> {
  const grouped = new Map<OperatorCategory, Operator[]>();

  for (const op of Object.values(operators)) {
    const existing = grouped.get(op.category) || [];
    existing.push(op);
    grouped.set(op.category, existing);
  }

  return grouped;
}

/**
 * Search operators by name or description
 * @param query - Search query (case-insensitive)
 * @returns Array of matching operators
 */
export function searchOperators(query: string): Operator[] {
  const lowerQuery = query.toLowerCase();
  return Object.values(operators).filter(
    (op) =>
      op.name.toLowerCase().includes(lowerQuery) ||
      op.label.toLowerCase().includes(lowerQuery) ||
      op.description.toLowerCase().includes(lowerQuery) ||
      op.help.summary.toLowerCase().includes(lowerQuery)
  );
}


