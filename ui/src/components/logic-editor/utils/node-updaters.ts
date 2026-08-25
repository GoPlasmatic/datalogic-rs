/**
 * Node Updaters
 *
 * Utilities for converting panel values back to node data.
 * This is the reverse of getInitialValuesFromNode in properties-panel/utils.ts
 *
 * Variable operators (var, val, exists) rebuild their cells together with the
 * expression so the two never diverge; a value the panel did not touch keeps
 * what the node already had.
 */

import type {
  LogicNodeData,
  LiteralNodeData,
  OperatorNodeData,
  VariableNodeData,
  CellData,
} from '../types';
import type { JsonLogicValue } from '../types/jsonlogic';
import { buildVariableCells } from './node-factory';
import {
  METADATA_SCOPE,
  isVariableOperatorName,
  pathComponentsFromCellValue,
  rawOperandOf,
  variableCellsToExpression,
} from './converters/variable-cells';
import { isSimpleOperand } from './type-helpers';

/**
 * Convert panel values back to node data.
 * Updates only the fields that can be edited via the panel.
 */
export function panelValuesToNodeData(
  currentData: LogicNodeData,
  panelValues: Record<string, unknown>
): LogicNodeData {
  switch (currentData.type) {
    case 'literal':
      return literalPanelToData(currentData, panelValues);
    case 'operator': {
      // Check if this is a variable operator (var, val, exists) which has editable fields
      const opData = currentData as OperatorNodeData;
      if (isVariableOperatorName(opData.operator)) {
        return variablePanelToData(currentData as VariableNodeData, panelValues);
      }
      // Other operator types don't have editable panel fields (their children are edited separately)
      return currentData;
    }
    case 'structure':
      // Structure nodes don't have editable panel fields
      return currentData;
    default:
      return currentData;
  }
}

/**
 * Convert literal panel values to LiteralNodeData
 */
function literalPanelToData(
  currentData: LiteralNodeData,
  panelValues: Record<string, unknown>
): LiteralNodeData {
  const valueType = (panelValues.valueType as LiteralNodeData['valueType']) ?? currentData.valueType;
  let value: JsonLogicValue;

  switch (valueType) {
    case 'string':
      value = String(panelValues.value ?? '');
      break;
    case 'number': {
      const num = Number(panelValues.value);
      value = isNaN(num) ? 0 : num;
      break;
    }
    case 'boolean':
      value = Boolean(panelValues.value);
      break;
    case 'null':
      value = null;
      break;
    case 'array':
      // For arrays, value stays as-is (complex editing not yet supported)
      value = currentData.value;
      break;
    default:
      value = currentData.value;
  }

  return {
    ...currentData,
    valueType,
    value,
    expression: value, // Update expression to match new value
  };
}

/**
 * Convert variable panel values to VariableNodeData
 */
function variablePanelToData(
  currentData: VariableNodeData,
  panelValues: Record<string, unknown>
): VariableNodeData {
  switch (currentData.operator) {
    case 'var':
      return varPanelToData(currentData, panelValues);
    case 'val':
      return valPanelToData(currentData, panelValues);
    case 'exists':
      return existsPanelToData(currentData, panelValues);
    default:
      return currentData;
  }
}

/** The stored operands of the node's expression, normalized to an array. */
function storedOperands(data: OperatorNodeData): JsonLogicValue[] {
  const raw = rawOperandOf(data.expression);
  if (raw === undefined) return [];
  return Array.isArray(raw) ? raw : [raw];
}

/**
 * Convert var panel values to VariableNodeData
 * Panel fields: path, hasDefault, default
 */
function varPanelToData(
  currentData: VariableNodeData,
  panelValues: Record<string, unknown>
): VariableNodeData {
  const cells = currentData.cells ?? [];
  const pathCell = cells.find((c) => c.fieldId === 'path');
  const defaultCell = cells.find((c) => c.index === 1 && c.fieldId !== 'path');
  const operands = storedOperands(currentData);

  const currentPath = pathCell?.value ?? currentData.path ?? '';
  const rawPath = panelValues.path ?? currentPath;
  const path: string | number = typeof rawPath === 'number' ? rawPath : String(rawPath ?? '');

  const hadDefault = defaultCell !== undefined;
  const currentDefault: JsonLogicValue | undefined = hadDefault
    ? (defaultCell?.type === 'editable'
      ? (defaultCell.value as JsonLogicValue)
      : (operands[1] ?? currentData.defaultValue ?? null))
    : undefined;
  const hasDefault = panelValues.hasDefault === undefined ? hadDefault : Boolean(panelValues.hasDefault);
  const defaultValue: JsonLogicValue | undefined = hasDefault
    ? ((panelValues.default as JsonLogicValue | undefined) ?? currentDefault ?? null)
    : undefined;

  // A complex default stays wired as a child; only rebuild cells when the
  // default is simple (inline) or removed.
  const keepBranchDefault =
    hasDefault &&
    defaultCell?.type === 'branch' &&
    (panelValues.default === undefined || !isSimpleOperand(defaultValue as JsonLogicValue));

  let newCells: CellData[];
  if (keepBranchDefault) {
    newCells = buildVariableCells({ operator: 'var', path });
    newCells.push(defaultCell as CellData);
  } else {
    newCells = buildVariableCells({ operator: 'var', path, defaultValue });
  }

  const expression: JsonLogicValue =
    hasDefault ? { var: [path, defaultValue === undefined ? null : defaultValue] } : { var: path };

  return {
    ...currentData,
    path: String(path),
    defaultValue,
    cells: newCells,
    expression,
    expressionText: undefined,
  };
}

/**
 * Convert val panel values to VariableNodeData
 * Panel fields: accessType, scopeLevel, path (array or dotted string), metadataKey
 */
function valPanelToData(
  currentData: VariableNodeData,
  panelValues: Record<string, unknown>
): VariableNodeData {
  const cells = currentData.cells ?? [];
  const scopeCell = cells.find((c) => c.fieldId === 'scopeLevel');
  const pathCells = cells.filter((c) => c.fieldId === 'path');
  const accessType = String(panelValues.accessType ?? 'path');

  let scope: number;
  let components: JsonLogicValue[];

  if (accessType === 'metadata') {
    // Iteration metadata is only reachable through the frame scope:
    // {"val": [[1], "index"]} / {"val": [[1], "key"]} ({"val": "index"} is a plain key lookup).
    const metadataKey = String(panelValues.metadataKey ?? 'index');
    scope = METADATA_SCOPE;
    components = [metadataKey];
  } else {
    const currentScope = typeof scopeCell?.value === 'number' ? scopeCell.value : (currentData.scopeJump ?? 0);
    scope = panelValues.scopeLevel === undefined ? currentScope : (Number(panelValues.scopeLevel) || 0);

    if (Array.isArray(panelValues.path)) {
      components = (panelValues.path as unknown[]).map((p) => (typeof p === 'number' ? p : String(p)));
    } else if (typeof panelValues.path === 'string') {
      components = pathComponentsFromCellValue(panelValues.path);
    } else if (pathCells.length > 0) {
      components = pathCells.flatMap((pc) => pathComponentsFromCellValue(pc.value));
    } else {
      components = currentData.pathComponents ?? [];
    }
  }

  const newCells = buildVariableCells({ operator: 'val', path: '', scopeJump: scope, pathComponents: components });
  const expression =
    variableCellsToExpression('val', newCells, { rawOperand: rawOperandOf(currentData.expression) }) ??
    { val: [] };

  return {
    ...currentData,
    path: components.map(String).join('.'),
    pathComponents: components.map(String),
    scopeJump: scope !== 0 ? scope : undefined,
    cells: newCells,
    expression,
    expressionText: undefined,
  };
}

/**
 * Convert exists panel values to VariableNodeData
 * Panel fields: pathType, dotPath, arrayPath
 *
 * The engine treats a string argument as one literal key, so a dotted path
 * typed in the panel is emitted as the array form (the only form that walks
 * nested objects).
 */
function existsPanelToData(
  currentData: VariableNodeData,
  panelValues: Record<string, unknown>
): VariableNodeData {
  const cells = currentData.cells ?? [];
  const pathCell = cells.find((c) => c.fieldId === 'path');
  const currentValue = pathCell?.value ?? currentData.pathComponents ?? currentData.path ?? '';
  const pathType = String(panelValues.pathType ?? (Array.isArray(currentValue) ? 'array' : 'dot'));

  let path: string | number = '';
  let components: JsonLogicValue[] | undefined;

  if (pathType === 'array') {
    if (Array.isArray(panelValues.arrayPath)) {
      components = (panelValues.arrayPath as unknown[]).map((p) => (typeof p === 'number' ? p : String(p)));
    } else if (Array.isArray(currentValue)) {
      components = [...(currentValue as JsonLogicValue[])];
    } else {
      components = pathComponentsFromCellValue(currentValue);
    }
  } else if (
    panelValues.dotPath === undefined ||
    (!Array.isArray(currentValue) && String(panelValues.dotPath) === String(currentValue))
  ) {
    // Untouched: keep whatever the node holds (a literal key may contain '.').
    if (Array.isArray(currentValue)) {
      components = [...(currentValue as JsonLogicValue[])];
    } else {
      path = typeof currentValue === 'number' ? currentValue : String(currentValue);
    }
  } else {
    const dotPath = String(panelValues.dotPath);
    if (dotPath.includes('.')) {
      components = pathComponentsFromCellValue(dotPath);
    } else {
      path = dotPath;
    }
  }

  const newCells = buildVariableCells({ operator: 'exists', path, pathComponents: components });
  const expression =
    variableCellsToExpression('exists', newCells, { rawOperand: rawOperandOf(currentData.expression) }) ??
    { exists: path };

  return {
    ...currentData,
    path: components ? components.map(String).join('.') : String(path),
    pathComponents: components?.map(String),
    cells: newCells,
    expression,
    expressionText: undefined,
  };
}

/**
 * Check if panel values have changed compared to node data
 */
export function havePanelValuesChanged(
  nodeData: LogicNodeData,
  panelValues: Record<string, unknown>
): boolean {
  const updatedData = panelValuesToNodeData(nodeData, panelValues);
  return !nodeEditsEqual(nodeData, updatedData);
}

/**
 * Compare the editable parts of two node data objects (expression, cells and
 * literal value). Presentation fields are ignored so a no-op panel apply does
 * not register as an edit.
 */
export function nodeEditsEqual(a: LogicNodeData, b: LogicNodeData): boolean {
  const pick = (d: LogicNodeData) => ({
    expression: d.expression ?? null,
    cells: d.type === 'operator' ? (d as OperatorNodeData).cells : undefined,
    value: d.type === 'literal' ? (d as LiteralNodeData).value : undefined,
    valueType: d.type === 'literal' ? (d as LiteralNodeData).valueType : undefined,
  });
  return JSON.stringify(pick(a)) === JSON.stringify(pick(b));
}
