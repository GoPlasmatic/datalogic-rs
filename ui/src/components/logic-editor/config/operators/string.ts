/**
 * String Operators
 *
 * Text manipulation operations.
 * - cat: Concatenate strings
 * - substr: Extract substring
 * - in: Check if substring exists
 * - length: Get string length
 * - starts_with, ends_with: Check prefix/suffix
 * - upper, lower, trim: Transform strings
 * - split: Split string into array
 */

import { stringCoreOperators } from './string-core';
import { stringTransformOperators } from './string-transform';

export const stringOperators = {
  ...stringCoreOperators,
  ...stringTransformOperators,
};
