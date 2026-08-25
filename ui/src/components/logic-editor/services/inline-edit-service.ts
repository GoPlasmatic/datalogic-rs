/**
 * Inline Edit Service
 *
 * Pure functions behind the properties panel's inline argument inputs: given
 * an operator node's data and a cell index, produce the data patch that
 * applies a new inline value. Knows the three cell layouts whose indices do
 * not map 1:1 onto the stored operands (variable cells, switch rows) and the
 * generic n-ary layout that does.
 */

import type { OperatorNodeData, JsonLogicValue, CellData } from '../types';
import { formatOperandLabel } from '../utils/formatting';
import {
  isSwitchOperator,
  isSwitchCells,
  resolveSwitchCellValues,
  switchArgsFromCells,
  switchStoredValueForCell,
} from '../utils/converters/switch-cells';
import {
  isVariableOperatorName,
  joinPathComponents,
  pathComponentsFromCellValue,
  rawOperandOf,
  scopeLabel,
  variableCellsToExpression,
  varPathLabel,
} from '../utils/converters/variable-cells';

/** The stored operands of a node's expression, normalized to an array. */
function storedOperandsOf(data: OperatorNodeData): JsonLogicValue[] {
  const raw = rawOperandOf(data.expression);
  if (raw === undefined) return [];
  return Array.isArray(raw) ? raw : [raw];
}

/**
 * The inline (stored) operand a cell displays, looked up the way the
 * serializer would: switch rows through their Case/Then pairing, everything
 * else by cell index. Undefined when the node has no stored operand for it.
 */
export function inlineOperandForCell(data: OperatorNodeData, cell: CellData): JsonLogicValue | undefined {
  const stored = storedOperandsOf(data);
  if (isSwitchOperator(data.operator) && isSwitchCells(data.cells)) {
    return switchStoredValueForCell(data.cells, stored, cell);
  }
  return stored[cell.index];
}

/** Apply a typed value to an editable variable cell (path / scope / default). */
function updateEditableCell(operator: string, cell: CellData, newValue: JsonLogicValue): CellData {
  if (cell.fieldId === 'scopeLevel') {
    const scope = typeof newValue === 'number' ? newValue : Number(newValue) || 0;
    return { ...cell, value: scope, label: scopeLabel(scope) };
  }
  if (cell.fieldId === 'path' && (operator === 'val' || operator === 'exists')) {
    // A typed string is dot notation; keep components as an array so the
    // engine receives the nested-path form.
    const components = pathComponentsFromCellValue(newValue);
    return { ...cell, value: components, label: joinPathComponents(components) };
  }
  if (cell.fieldId === 'path') {
    return { ...cell, value: newValue, label: varPathLabel(newValue) };
  }
  return { ...cell, value: newValue, label: formatOperandLabel(newValue) };
}

/**
 * Compute the data patch for changing the inline value shown at `cellIndex`.
 * Returns null when the cell is unknown or holds a wired child.
 */
export function updateInlineOperand(
  data: OperatorNodeData,
  cellIndex: number,
  newValue: JsonLogicValue
): Partial<OperatorNodeData> | null {
  const cell = data.cells.find((c) => c.index === cellIndex);
  if (!cell) return null;
  if (cell.type === 'branch') return null;

  // Editable variable cells (var path, val scope / path, exists path, editable default)
  if (cell.type === 'editable' && isVariableOperatorName(data.operator)) {
    const cells = data.cells.map((c) => (c.index === cellIndex ? updateEditableCell(data.operator, c, newValue) : c));
    const expression =
      variableCellsToExpression(data.operator, cells, { rawOperand: rawOperandOf(data.expression) }) ??
      data.expression ??
      null;
    return { cells, expression, expressionText: undefined };
  }

  const stored = storedOperandsOf(data);
  const relabel = (c: CellData): CellData =>
    c.index === cellIndex
      ? { ...c, label: formatOperandLabel(newValue), ...(c.fieldId ? { value: newValue } : {}) }
      : c;

  // Switch rows: Case / Then / Default map onto the nested cases array
  if (isSwitchOperator(data.operator) && isSwitchCells(data.cells)) {
    const values = resolveSwitchCellValues(data.cells, stored);
    values.set(cellIndex, newValue);
    const args = switchArgsFromCells(data.cells, values, stored.length >= 2);
    return {
      cells: data.cells.map(relabel),
      expression: { [data.operator]: args },
      expressionText: undefined,
    };
  }

  // Generic: the cell index is the operand index
  const operands = [...stored];
  operands[cellIndex] = newValue;
  return {
    cells: data.cells.map(relabel),
    expression: { [data.operator]: operands },
    expressionText: undefined,
  };
}
