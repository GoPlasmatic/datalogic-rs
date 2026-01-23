import type { JsonLogicValue } from '../../types';
import type { ConversionContext } from './types';
import { isPlainObject, isSimpleOperand, isDataStructure } from '../type-helpers';
import { generateExpressionText } from '../formatting';
import { convertPrimitive, convertInvalidObject } from './primitive-converter';
import { isVariableOperator, convertVariable } from './variable-converter';
import { convertIfElse } from './if-else-converter';
import {
  convertToVerticalCell,
  convertUnaryInline,
  convertOperatorWithChildren,
  isUnaryOperator,
} from './operator-converter';
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

  // Handle if/else - convert to vertical cell node
  if (operator === 'if' || operator === '?:') {
    const ifArgs: JsonLogicValue[] = Array.isArray(operands) ? operands : [operands];
    return convertIfElse(ifArgs, context, convertValue);
  }

  // Handle variable operators specially
  if (isVariableOperator(operator)) {
    return convertVariable(operator, operands, context);
  }

  // Normalize operands to array
  const operandArray: JsonLogicValue[] = Array.isArray(operands) ? operands : [operands];

  // Use VerticalCellNode for ALL operators with more than 1 argument
  if (operandArray.length > 1) {
    return convertToVerticalCell(operator, operandArray, context, convertValue);
  }

  // For unary operators (single arg), use standard operator node
  const expressionText = generateExpressionText(value);

  // Check if this is a unary operator with a simple operand - show inline without expansion
  const singleOperand = operandArray[0];
  const isUnaryWithSimpleArg = isUnaryOperator(operator) && isSimpleOperand(singleOperand);

  if (isUnaryWithSimpleArg) {
    return convertUnaryInline(operator, expressionText, value, context);
  }

  // For other single-arg operators, create with child node
  return convertOperatorWithChildren(operator, operandArray, value, context, convertValue);
}
