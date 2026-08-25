import type { JsonLogicValue, CellData } from '../../types';
import { rawOperandOf, variableCellsToExpression } from '../../utils/converters/variable-cells';

/**
 * Rebuild expression for variable operators (var, val, exists) from editable cell values.
 *
 * `currentExpression` (the node's stored expression) supplies the original
 * argument form (string vs array) and the default of a var whose default is
 * wired to a child or stored inline.
 */
export function rebuildVariableExpression(
  operator: string,
  cells: CellData[],
  currentExpression?: JsonLogicValue
): JsonLogicValue {
  const rebuilt = variableCellsToExpression(operator, cells, {
    rawOperand: rawOperandOf(currentExpression),
  });
  if (rebuilt !== undefined) return rebuilt;
  return currentExpression ?? { [operator]: [] };
}
