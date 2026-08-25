/**
 * Array Operators
 *
 * Array operations and iteration.
 * - Iteration: map, filter, reduce, all, some, none
 * - Manipulation: merge, sort, slice, group_by, distinct
 */

import { arrayIterationOperators } from './array-iteration';
import { arrayManipulationOperators } from './array-manipulation';

export const arrayOperators = {
  ...arrayIterationOperators,
  ...arrayManipulationOperators,
};
