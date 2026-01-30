import type { JsonLogicValue } from '../../types';
import type { ConversionContext } from './types';
import { isPlainObject, isDataStructure } from '../type-helpers';
import { convertPrimitive, convertInvalidObject } from './primitive-converter';
import { isVariableOperator, convertVariable } from './variable-converter';
import { convertIfElse } from './if-else-converter';
import { convertOperator } from './operator-converter';
import { convertStructure } from './structure-converter';

export type { ConversionContext, ParentInfo, ConverterFn } from './types';

// Main conversion function - converts a JSONLogic value to nodes
export function convertValue(
  value: JsonLogicValue,
  context: ConversionContext
): string {
  // In preserveStructure mode, check for data structures (multi-key objects or arrays with content)
  if (context.preserveStructure && isDataStructure(value)) {
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

  // Handle if/else
  if (operator === 'if' || operator === '?:') {
    const ifArgs: JsonLogicValue[] = Array.isArray(operands) ? operands : [operands];
    return convertIfElse(ifArgs, context, convertValue);
  }

  // Handle variable operators
  if (isVariableOperator(operator)) {
    return convertVariable(operator, operands, context, convertValue);
  }

  // Normalize operands to array
  const operandArray: JsonLogicValue[] = Array.isArray(operands) ? operands : [operands];

  // All operators use the unified convertOperator - produces cells-based nodes
  return convertOperator(operator, operandArray, context, convertValue);
}
