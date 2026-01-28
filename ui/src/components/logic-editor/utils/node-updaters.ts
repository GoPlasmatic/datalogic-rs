/**
 * Node Updaters
 *
 * Utilities for converting panel values back to node data.
 * This is the reverse of getInitialValuesFromNode in properties-panel/utils.ts
 */

import type {
  LogicNodeData,
  LiteralNodeData,
  VariableNodeData,
} from '../types';
import type { JsonLogicValue } from '../types/jsonlogic';

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
    case 'variable':
      return variablePanelToData(currentData, panelValues);
    case 'operator':
    case 'verticalCell':
    case 'decision':
    case 'structure':
      // These node types don't have editable panel fields (their children are edited separately)
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

/**
 * Convert var panel values to VariableNodeData
 * Panel fields: path, hasDefault, default
 */
function varPanelToData(
  currentData: VariableNodeData,
  panelValues: Record<string, unknown>
): VariableNodeData {
  const path = String(panelValues.path ?? currentData.path);
  const hasDefault = Boolean(panelValues.hasDefault);
  const defaultValue = hasDefault ? (panelValues.default as JsonLogicValue) : undefined;

  // Build the expression
  let expression: JsonLogicValue;
  if (hasDefault) {
    expression = { var: [path, defaultValue] };
  } else {
    expression = { var: path };
  }

  return {
    ...currentData,
    path,
    defaultValue,
    expression,
  };
}

/**
 * Convert val panel values to VariableNodeData
 * Panel fields: accessType, scopeLevel, path (array), metadataKey
 */
function valPanelToData(
  currentData: VariableNodeData,
  panelValues: Record<string, unknown>
): VariableNodeData {
  const accessType = String(panelValues.accessType ?? 'path');

  if (accessType === 'metadata') {
    // Metadata access: {val: "index"} or {val: "key"}
    const metadataKey = String(panelValues.metadataKey ?? 'index');
    return {
      ...currentData,
      path: metadataKey,
      pathComponents: [metadataKey],
      scopeJump: undefined,
      expression: { val: metadataKey },
    };
  }

  // Path access: {val: [[-N], "path", "components"]}
  const scopeLevel = Number(panelValues.scopeLevel ?? 0);
  const pathComponents = Array.isArray(panelValues.path)
    ? (panelValues.path as string[]).map(String)
    : currentData.pathComponents ?? [];

  // Build the expression
  let expressionArgs: JsonLogicValue[];
  if (scopeLevel > 0) {
    // Include scope jump array: [[-N], "path", "components"]
    expressionArgs = [[-scopeLevel], ...pathComponents];
  } else if (pathComponents.length === 0) {
    // Empty path - current element
    expressionArgs = [];
  } else {
    // No scope jump, just path components
    expressionArgs = pathComponents;
  }

  return {
    ...currentData,
    path: pathComponents.join('.'),
    pathComponents,
    scopeJump: scopeLevel > 0 ? scopeLevel : undefined,
    expression: { val: expressionArgs },
  };
}

/**
 * Convert exists panel values to VariableNodeData
 * Panel fields: pathType, dotPath, arrayPath
 */
function existsPanelToData(
  currentData: VariableNodeData,
  panelValues: Record<string, unknown>
): VariableNodeData {
  const pathType = String(panelValues.pathType ?? 'dot');

  if (pathType === 'dot') {
    const dotPath = String(panelValues.dotPath ?? currentData.path);
    return {
      ...currentData,
      path: dotPath,
      pathComponents: undefined,
      expression: { exists: dotPath },
    };
  }

  // Array path
  const arrayPath = Array.isArray(panelValues.arrayPath)
    ? (panelValues.arrayPath as string[]).map(String)
    : [];

  return {
    ...currentData,
    path: arrayPath.join('.'),
    pathComponents: arrayPath,
    expression: { exists: arrayPath },
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
  return JSON.stringify(nodeData.expression) !== JSON.stringify(updatedData.expression);
}
