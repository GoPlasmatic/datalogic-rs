/**
 * Arithmetic Operators
 *
 * Mathematical operations.
 * - Basic: +, -, *, /, %
 * - Aggregate: max, min
 * - Unary: abs, ceil, floor
 */

import { arithmeticBasicOperators } from './arithmetic-basic';
import { arithmeticFunctionOperators } from './arithmetic-functions';

export const arithmeticOperators = {
  ...arithmeticBasicOperators,
  ...arithmeticFunctionOperators,
};
