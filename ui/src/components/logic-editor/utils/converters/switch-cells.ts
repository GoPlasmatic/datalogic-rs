/**
 * Switch / match cell helpers
 *
 * A switch node flattens {"switch": [discriminant, [[case, result], ...], default]}
 * into rows: Match(0), Case(1), Then(2), Case(3), Then(4), ..., Default(N).
 * Cell indices therefore do NOT line up with the three operands, so every
 * place that reads or writes an inline value must go through these helpers.
 */

import type { JsonLogicValue, CellData } from '../../types';

export const SWITCH_ROW_LABELS = ['Match', 'Case', 'Then', 'Default'] as const;
export type SwitchRowLabel = (typeof SWITCH_ROW_LABELS)[number];

export function isSwitchOperator(operator: string): boolean {
  return operator === 'switch' || operator === 'match';
}

/** True when the cells carry the Match/Case/Then/Default row layout. */
export function isSwitchCells(cells: CellData[]): boolean {
  return (
    cells.length > 0 &&
    cells.every((c) => (SWITCH_ROW_LABELS as readonly string[]).includes(c.rowLabel ?? ''))
  );
}

/** A cases operand is well formed when every entry is a [case, result] pair. */
export function isWellFormedCases(cases: JsonLogicValue): cases is JsonLogicValue[][] {
  return Array.isArray(cases) && cases.every((pair) => Array.isArray(pair) && pair.length === 2);
}

/** Split [discriminant, cases, default] out of a stored operand list. */
export function splitSwitchOperands(storedOperands: JsonLogicValue[]): {
  discriminant: JsonLogicValue;
  cases: JsonLogicValue[][];
  defaultValue: JsonLogicValue | undefined;
  hasCases: boolean;
} {
  const casesOperand = storedOperands[1] ?? null;
  const cases: JsonLogicValue[][] = isWellFormedCases(casesOperand) ? casesOperand : [];
  return {
    discriminant: storedOperands[0] ?? null,
    cases,
    defaultValue: storedOperands.length >= 3 ? storedOperands[2] : undefined,
    hasCases: storedOperands.length >= 2,
  };
}

/**
 * The stored (inline) operand every switch cell corresponds to, by row
 * position: Match -> operands[0], the k-th Case/Then -> cases[k][0|1],
 * Default -> operands[2]. One pass, keyed by cell index.
 */
export function switchStoredValues(
  cells: CellData[],
  storedOperands: JsonLogicValue[]
): Map<number, JsonLogicValue | undefined> {
  const { discriminant, cases, defaultValue } = splitSwitchOperands(storedOperands);
  const stored = new Map<number, JsonLogicValue | undefined>();
  let caseIndex = -1;
  for (const cell of cells) {
    if (cell.rowLabel === 'Case') caseIndex++;
    if (stored.has(cell.index)) continue;
    switch (cell.rowLabel) {
      case 'Match':
        stored.set(cell.index, discriminant);
        break;
      case 'Case':
        stored.set(cell.index, cases[caseIndex]?.[0]);
        break;
      case 'Then':
        stored.set(cell.index, cases[caseIndex]?.[1]);
        break;
      case 'Default':
        stored.set(cell.index, defaultValue);
        break;
      default:
        stored.set(cell.index, undefined);
    }
  }
  return stored;
}

/** The stored operand for one cell (see [`switchStoredValues`]). */
export function switchStoredValueForCell(
  cells: CellData[],
  storedOperands: JsonLogicValue[],
  target: CellData
): JsonLogicValue | undefined {
  return switchStoredValues(cells, storedOperands).get(target.index);
}

/**
 * Resolve every cell to a value: branch cells through `resolveBranch`
 * (falling back to the stored operand), inline cells to the stored operand.
 * Returns a map keyed by cell index so callers can filter/reorder cells
 * afterwards without losing the alignment.
 */
export function resolveSwitchCellValues(
  cells: CellData[],
  storedOperands: JsonLogicValue[],
  resolveBranch?: (cell: CellData) => JsonLogicValue | undefined
): Map<number, JsonLogicValue> {
  const stored = switchStoredValues(cells, storedOperands);
  const values = new Map<number, JsonLogicValue>();
  for (const cell of cells) {
    let value: JsonLogicValue | undefined;
    if (cell.type === 'branch' && cell.branchId) {
      value = resolveBranch?.(cell);
    }
    if (value === undefined) value = stored.get(cell.index);
    values.set(cell.index, value === undefined ? null : value);
  }
  return values;
}

/**
 * Rebuild [discriminant, [[case, result], ...], default?] from cells and their
 * resolved values. The cases array is emitted when there is at least one
 * Case/Then pair or the original rule carried one (so {"switch": [d]} stays
 * exactly that).
 */
export function switchArgsFromCells(
  cells: CellData[],
  values: Map<number, JsonLogicValue>,
  hadCases: boolean
): JsonLogicValue[] {
  let discriminant: JsonLogicValue = null;
  const pairs: JsonLogicValue[][] = [];
  let defaultValue: JsonLogicValue | undefined;
  let pendingCase: JsonLogicValue | undefined;
  let hasPendingCase = false;

  for (const cell of cells) {
    const value = values.get(cell.index) ?? null;
    if (cell.rowLabel === 'Match') {
      discriminant = value;
    } else if (cell.rowLabel === 'Case') {
      pendingCase = value;
      hasPendingCase = true;
    } else if (cell.rowLabel === 'Then') {
      pairs.push([hasPendingCase ? (pendingCase as JsonLogicValue) : null, value]);
      hasPendingCase = false;
      pendingCase = undefined;
    } else if (cell.rowLabel === 'Default') {
      defaultValue = value;
    }
  }
  if (hasPendingCase) {
    // A Case without its Then (should not happen); keep it rather than drop it.
    pairs.push([pendingCase as JsonLogicValue, null]);
  }

  const args: JsonLogicValue[] = [discriminant];
  if (pairs.length > 0 || hadCases || defaultValue !== undefined) {
    args.push(pairs as unknown as JsonLogicValue);
  }
  if (defaultValue !== undefined) args.push(defaultValue);
  return args;
}

/** Find the Case/Then partner of a switch cell (the adjacent row of its pair). */
export function switchPairPartner(cells: CellData[], cell: CellData): CellData | undefined {
  const pos = cells.findIndex((c) => c.index === cell.index);
  if (pos === -1) return undefined;
  if (cell.rowLabel === 'Case') {
    const next = cells[pos + 1];
    return next?.rowLabel === 'Then' ? next : undefined;
  }
  if (cell.rowLabel === 'Then') {
    const prev = cells[pos - 1];
    return prev?.rowLabel === 'Case' ? prev : undefined;
  }
  return undefined;
}
