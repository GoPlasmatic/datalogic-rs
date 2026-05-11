/**
 * Visibility Condition Evaluation
 *
 * Utilities for evaluating showWhen conditions on fields and sections.
 */

import type { VisibilityCondition } from '../config/operators.types';

/**
 * Evaluates a single visibility condition against the current form values.
 */
export function evaluateCondition(
  condition: VisibilityCondition,
  values: Record<string, unknown>
): boolean {
  const fieldValue = values[condition.field];

  switch (condition.operator) {
    case 'equals':
      return fieldValue === condition.value;

    case 'notEquals':
      return fieldValue !== condition.value;

    case 'exists':
      return fieldValue !== undefined && fieldValue !== null;

    case 'notExists':
      return fieldValue === undefined || fieldValue === null;

    default:
      return true;
  }
}

/**
 * Evaluates multiple visibility conditions (AND logic - all must be true).
 */
export function evaluateConditions(
  conditions: VisibilityCondition[] | undefined,
  values: Record<string, unknown>
): boolean {
  if (!conditions || conditions.length === 0) {
    return true;
  }

  return conditions.every((condition) => evaluateCondition(condition, values));
}
