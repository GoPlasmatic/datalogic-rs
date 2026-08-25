/**
 * Variable cell helpers (var / val / exists)
 *
 * The single place that knows how a variable operator's operands map onto its
 * editable cells and back. Used by the JSONLogic -> nodes converter, the
 * nodes -> JSONLogic serializer, the argument/inline-edit services and the
 * properties panel so the three never disagree about the shape of a path.
 *
 * Cell layout:
 *   var    [0] path (editable, fieldId 'path', value: string | number)
 *          [1] default (inline or editable, fieldId 'default') or a branch cell
 *   val    [0] scope (editable, fieldId 'scopeLevel', value: number, sign kept)
 *          [1..] path (editable, fieldId 'path', value: JsonLogicValue[])
 *   exists [0] path (editable, fieldId 'path', value: string | JsonLogicValue[])
 *
 * A val/exists path cell keeps its components as an array so keys containing
 * '.' survive; a string value (typed by the user) is split on '.'.
 */

import type { JsonLogicValue, CellData } from '../../types';
import { isPlainObject } from '../type-helpers';
import { CONTEXT_LABEL } from '../formatting';

export const VARIABLE_OPERATORS = ['var', 'val', 'exists'] as const;
export type VariableOperator = (typeof VARIABLE_OPERATORS)[number];

export function isVariableOperatorName(operator: string): operator is VariableOperator {
  return (VARIABLE_OPERATORS as readonly string[]).includes(operator);
}

/** A path component the editor can hold in a cell: string or number. */
function isStaticComponent(value: unknown): value is string | number {
  return typeof value === 'string' || typeof value === 'number';
}

export interface ParsedVar {
  /** false when the path (or the operand as a whole) is a computed expression */
  isStatic: boolean;
  path: string | number;
  defaultValue?: JsonLogicValue;
}

/** Parse the operand of {"var": ...}. */
export function parseVarOperand(raw: JsonLogicValue | undefined): ParsedVar {
  if (Array.isArray(raw)) {
    const first = raw.length > 0 ? raw[0] : '';
    if (!isStaticComponent(first) && first !== null) {
      return { isStatic: false, path: '' };
    }
    return {
      isStatic: true,
      path: first === null ? '' : first,
      defaultValue: raw.length > 1 ? raw[1] : undefined,
    };
  }
  if (raw === undefined || raw === null) return { isStatic: true, path: '' };
  if (isStaticComponent(raw)) return { isStatic: true, path: raw };
  return { isStatic: false, path: '' };
}

export interface ParsedVal {
  isStatic: boolean;
  /** Scope jump as written (sign preserved); 0 when no scope array is present */
  scope: number;
  components: JsonLogicValue[];
}

/** Parse the operand of {"val": ...}: optional leading [n] scope array, then path segments. */
export function parseValOperand(raw: JsonLogicValue | undefined): ParsedVal {
  if (raw === undefined || raw === null) return { isStatic: true, scope: 0, components: [] };
  if (isStaticComponent(raw)) return { isStatic: true, scope: 0, components: [raw] };
  if (!Array.isArray(raw)) return { isStatic: false, scope: 0, components: [] };

  let scope = 0;
  let rest: JsonLogicValue[] = raw;
  const first = raw[0];
  if (Array.isArray(first)) {
    if (first.length !== 1 || typeof first[0] !== 'number') {
      return { isStatic: false, scope: 0, components: [] };
    }
    scope = first[0];
    rest = raw.slice(1);
  }
  if (!rest.every(isStaticComponent)) return { isStatic: false, scope: 0, components: [] };
  return { isStatic: true, scope, components: rest };
}

export interface ParsedExists {
  isStatic: boolean;
  /** string / number = a single literal key; array = nested path components */
  path: string | number | JsonLogicValue[];
}

/** Parse the operand of {"exists": ...}. */
export function parseExistsOperand(raw: JsonLogicValue | undefined): ParsedExists {
  if (raw === undefined || raw === null) return { isStatic: true, path: '' };
  if (typeof raw === 'string' || typeof raw === 'number') return { isStatic: true, path: raw };
  if (Array.isArray(raw)) {
    return raw.every(isStaticComponent)
      ? { isStatic: true, path: raw }
      : { isStatic: false, path: '' };
  }
  return { isStatic: false, path: '' };
}

/**
 * Normalize whatever a path cell holds into path components. Arrays are used
 * as-is (they came from the JSON), strings are split on '.' (typed by the
 * user), numbers are a single index segment.
 */
export function pathComponentsFromCellValue(value: unknown): JsonLogicValue[] {
  if (Array.isArray(value)) return [...(value as JsonLogicValue[])];
  if (typeof value === 'number') return [value];
  if (typeof value === 'string') return value.split('.').filter((s) => s !== '');
  return [];
}

/** Display text for a list of path components (dot-joined, or "context" when empty). */
export function joinPathComponents(components: JsonLogicValue[]): string {
  const text = components.map((c) => String(c ?? '')).join('.');
  return text || CONTEXT_LABEL;
}

/** Display text for a var path cell value. */
export function varPathLabel(value: unknown): string {
  const text = value === undefined || value === null ? '' : String(value);
  return text || CONTEXT_LABEL;
}

/** Label for a val scope cell. */
export function scopeLabel(scope: number): string {
  const n = Math.abs(scope);
  return `${n} level${n !== 1 ? 's' : ''} up`;
}

/** The raw operand of a stored {op: operand} expression, or undefined. */
export function rawOperandOf(expression: JsonLogicValue | undefined): JsonLogicValue | undefined {
  if (!isPlainObject(expression)) return undefined;
  const keys = Object.keys(expression);
  if (keys.length !== 1) return undefined;
  return expression[keys[0]] as JsonLogicValue;
}

export interface VariableExpressionOptions {
  /** The operand of the node's stored expression (decides string vs array form) */
  rawOperand?: JsonLogicValue;
  /** Resolve a branch cell to its child's expression (serializer); undefined = use the stored operand */
  resolveBranch?: (cell: CellData) => JsonLogicValue | undefined;
}

/** True when the cells carry the editable variable layout described above. */
export function hasVariableCells(cells: CellData[]): boolean {
  return cells.some((c) => c.type === 'editable' && c.fieldId !== undefined);
}

/**
 * Rebuild a var / val / exists expression from its cells.
 *
 * Returns undefined when the cells do not carry the variable layout (a dynamic
 * path converted as a generic operator, or a foreign cell shape) so the caller
 * can fall back to generic serialization.
 */
export function variableCellsToExpression(
  operator: string,
  cells: CellData[],
  options: VariableExpressionOptions = {}
): JsonLogicValue | undefined {
  const { rawOperand, resolveBranch } = options;
  const stored: JsonLogicValue[] =
    rawOperand === undefined ? [] : Array.isArray(rawOperand) ? rawOperand : [rawOperand];

  if (operator === 'var') {
    const pathCell = cells.find((c) => c.fieldId === 'path');
    if (!pathCell) return undefined;
    const path: JsonLogicValue = isStaticComponent(pathCell.value) ? pathCell.value : String(pathCell.value ?? '');

    const defaultCell = cells.find((c) => c.index === 1 && c.fieldId !== 'path');
    if (defaultCell) {
      let defaultValue: JsonLogicValue | undefined;
      if (defaultCell.type === 'branch' && defaultCell.branchId) {
        defaultValue = resolveBranch?.(defaultCell);
        if (defaultValue === undefined) defaultValue = stored[1];
      } else if (defaultCell.type === 'editable') {
        defaultValue = defaultCell.value as JsonLogicValue;
      } else {
        defaultValue = stored[1];
      }
      return { var: [path, defaultValue === undefined ? null : defaultValue] };
    }
    if (Array.isArray(rawOperand)) {
      // Preserve the array form the rule was written in ({"var": ["p"]}, {"var": []}).
      return { var: rawOperand.length === 0 && path === '' ? [] : [path] };
    }
    return { var: path };
  }

  if (operator === 'val') {
    const scopeCell = cells.find((c) => c.fieldId === 'scopeLevel');
    const pathCells = cells.filter((c) => c.fieldId === 'path');
    if (!scopeCell && pathCells.length === 0) return undefined;

    const scope = typeof scopeCell?.value === 'number' && Number.isFinite(scopeCell.value)
      ? scopeCell.value
      : Number(scopeCell?.value ?? 0) || 0;
    const components: JsonLogicValue[] = [];
    for (const pc of pathCells) {
      components.push(...pathComponentsFromCellValue(pc.value));
    }

    // {"val": "key"} stays a plain string key when it was written as one.
    if (
      scope === 0 &&
      typeof rawOperand === 'string' &&
      components.length === 1 &&
      typeof components[0] === 'string'
    ) {
      return { val: components[0] };
    }

    // A scope array is emitted when the jump is non-zero or the rule carried
    // one ({"val": [[0], "x"]} stays as written).
    const hadScopeArray = Array.isArray(rawOperand) && Array.isArray(rawOperand[0]);
    const args: JsonLogicValue[] = [];
    if (scope !== 0 || hadScopeArray) args.push([scope]);
    args.push(...components);
    return { val: args };
  }

  if (operator === 'exists') {
    const pathCell = cells.find((c) => c.fieldId === 'path');
    if (!pathCell) return undefined;
    if (cells.some((c) => c.index > 0)) {
      // Foreign shape (e.g. a trace-built node holding extra segments in a
      // second cell): the stored expression is the only faithful source.
      return undefined;
    }
    const value = pathCell.value;
    if (Array.isArray(value)) return { exists: [...(value as JsonLogicValue[])] };
    if (typeof value === 'number') return { exists: value };
    const text = String(value ?? '');
    if (typeof rawOperand !== 'string' && text.includes('.')) {
      // A typed dotted path means nested access; the engine only walks the array form.
      return { exists: pathComponentsFromCellValue(text) };
    }
    return { exists: text };
  }

  return undefined;
}

/** Metadata access ({"val": [[1], "index"]}) reads the current iteration frame. */
export const METADATA_SCOPE = 1;

/** True when scope + components describe iteration metadata access. */
export function isMetadataAccess(scope: number, components: JsonLogicValue[]): boolean {
  return (
    Math.abs(scope) === METADATA_SCOPE &&
    components.length === 1 &&
    (components[0] === 'index' || components[0] === 'key')
  );
}
