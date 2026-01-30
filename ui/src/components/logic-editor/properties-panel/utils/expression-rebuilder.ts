import type { JsonLogicValue, CellData } from '../../types';

/**
 * Rebuild expression for variable operators (var, val, exists) from editable cell values
 */
export function rebuildVariableExpression(operator: string, cells: CellData[]): JsonLogicValue {
  if (operator === 'val') {
    const scopeCell = cells.find((c) => c.fieldId === 'scopeLevel');
    const pathCells = cells.filter((c) => c.fieldId === 'path');
    const scopeJump = typeof scopeCell?.value === 'number' ? scopeCell.value : 0;

    const pathComponents: string[] = [];
    for (const pc of pathCells) {
      const pathStr = String(pc.value ?? '');
      if (pathStr) {
        pathStr.split('.').forEach((comp) => {
          if (comp) pathComponents.push(comp);
        });
      }
    }

    // Simple metadata access
    if (scopeJump === 0 && pathComponents.length === 1 &&
        (pathComponents[0] === 'index' || pathComponents[0] === 'key')) {
      return { val: pathComponents[0] };
    }

    const args: JsonLogicValue[] = [];
    if (scopeJump > 0) {
      args.push([-scopeJump]);
    }
    args.push(...pathComponents);
    return { val: args.length === 0 ? [] : args };
  }

  if (operator === 'var') {
    const pathCell = cells.find((c) => c.fieldId === 'path');
    const pathValue = String(pathCell?.value ?? '');
    const defaultCell = cells.find((c) => c.index === 1 && c.fieldId !== 'path');
    if (defaultCell) {
      // Has default - keep as array form
      // For inline/editable defaults, use stored value; for branch, expression is rebuilt by serializer
      return { var: [pathValue, defaultCell.value as JsonLogicValue ?? null] };
    }
    return { var: pathValue };
  }

  if (operator === 'exists') {
    const pathCell = cells.find((c) => c.fieldId === 'path');
    return { exists: String(pathCell?.value ?? '') };
  }

  return { [operator]: [] };
}
