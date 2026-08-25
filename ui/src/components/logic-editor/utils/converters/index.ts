import type { JsonLogicValue } from '../../types';
import type { ConversionContext } from './types';
import { isPlainObject, isDataStructure } from '../type-helpers';
import { convertPrimitive, convertInvalidObject } from './primitive-converter';
import { isVariableOperator, convertVariable } from './variable-converter';
import { convertIfElse, isIfOperator } from './if-else-converter';
import { convertSwitch } from './switch-converter';
import { convertOperator } from './operator-converter';
import { convertStructure } from './structure-converter';

export type { ConversionContext, ParentInfo, ConverterFn } from './types';

// Main conversion function - converts a JSONLogic value to nodes
export function convertValue(
  value: JsonLogicValue,
  context: ConversionContext
): string {
  // In templating mode, check for data structures (multi-key objects or arrays with content)
  if (context.templating && isDataStructure(value)) {
    return convertStructure(value as Record<string, unknown> | unknown[], context, convertValue);
  }

  // Handle primitives and arrays as literals
  if (!isPlainObject(value)) {
    return convertPrimitive(value, context);
  }

  // Handle objects (operators)
  const keys = Object.keys(value);
  if (keys.length !== 1) {
    // Invalid JSONLogic, treat as literal
    return convertInvalidObject(value, context);
  }

  const operator = keys[0];
  const operands = value[operator];

  // Normalize operands to array (the raw form is kept on the node's expression)
  const operandArray: JsonLogicValue[] = Array.isArray(operands) ? operands : [operands];

  // Handle if / ?: as a decision-diamond chain. Anything shorter than
  // condition + then (including the non-array shorthand the engine rejects)
  // is kept as a generic node so it serializes back exactly as written.
  if (isIfOperator(operator)) {
    if (Array.isArray(operands) && operands.length >= 2) {
      return convertIfElse(operator, operands, context, convertValue);
    }
    return convertOperator(operator, operandArray, context, convertValue, operands);
  }

  // Handle switch/match
  if (operator === 'switch' || operator === 'match') {
    return convertSwitch(operator, operandArray, context, convertValue, operands);
  }

  // Handle variable operators
  if (isVariableOperator(operator)) {
    return convertVariable(operator, operands, context, convertValue);
  }

  // All operators use the unified convertOperator - produces cells-based nodes
  return convertOperator(operator, operandArray, context, convertValue, operands);
}
