/**
 * Properties Panel Utilities
 *
 * Helper functions for mapping node data to panel configurations.
 */

import type {
  LogicNodeData,
  LiteralNodeData,
  OperatorNodeData,
  StructureNodeData,
  JsonLogicValue,
} from '../types';
import type { Operator, PanelConfig } from '../config/operators.types';
import { getOperator } from '../config/operators';
import { literalPanelConfig, structurePanelConfig } from '../config/literalPanel';
import { isSimpleOperand } from '../utils/type-helpers';
import {
  hasVariableCells,
  isMetadataAccess,
  isVariableOperatorName,
  pathComponentsFromCellValue,
  rawOperandOf,
} from '../utils/converters/variable-cells';

/**
 * Get the panel configuration for a node
 */
export function getPanelConfigForNode(data: LogicNodeData): PanelConfig | null {
  switch (data.type) {
    case 'operator':
      return getOperatorPanelConfig(data);
    case 'literal':
      return literalPanelConfig;
    case 'structure':
      return getStructurePanelConfig();
    default:
      return null;
  }
}

/**
 * Get the operator config for a node (if applicable)
 */
export function getOperatorConfigForNode(data: LogicNodeData): Operator | null {
  switch (data.type) {
    case 'operator':
      return getOperator((data as OperatorNodeData).operator) ?? null;
    default:
      return null;
  }
}

function getOperatorPanelConfig(data: OperatorNodeData): PanelConfig | null {
  const op = getOperator(data.operator);
  return op?.panel ?? null;
}

function getStructurePanelConfig(): PanelConfig | null {
  // Structure nodes are the only place the `object` literal type applies;
  // `literalPanelConfig` deliberately omits it (LiteralNodeData has no
  // `object` valueType).
  return structurePanelConfig;
}

/**
 * Extract initial panel values from node data
 */
export function getInitialValuesFromNode(data: LogicNodeData): Record<string, unknown> {
  switch (data.type) {
    case 'operator':
      return getOperatorInitialValues(data);
    case 'literal':
      return getLiteralInitialValues(data);
    case 'structure':
      return getStructureInitialValues(data);
    default:
      return {};
  }
}

function getOperatorInitialValues(data: OperatorNodeData): Record<string, unknown> {
  const raw = rawOperandOf(data.expression);
  const operands: JsonLogicValue[] = raw === undefined ? [] : Array.isArray(raw) ? raw : [raw];

  // A var / val / exists with a computed path (an expression where a path
  // segment is expected) is a generic operator node: it has no path cells to
  // seed from and its operands are edited through the arguments section.
  if (isVariableOperatorName(data.operator) && !hasVariableCells(data.cells)) {
    return {};
  }

  // For variable operators, extract values from editable cells
  if (data.operator === 'var') {
    const pathCell = data.cells.find((c) => c.fieldId === 'path');
    // The default is the second cell: inline (value stored on the expression),
    // editable, or a wired child (complex default).
    const defaultCell = data.cells.find((c) => c.index === 1 && c.fieldId !== 'path');
    let defaultValue: unknown;
    if (defaultCell?.type === 'editable') {
      defaultValue = defaultCell.value;
    } else if (defaultCell && defaultCell.type !== 'branch') {
      defaultValue = operands[1];
    } else if (defaultCell && isSimpleOperand(operands[1] ?? null) && operands.length > 1) {
      defaultValue = operands[1];
    }
    return {
      path: pathCell?.value ?? '',
      hasDefault: defaultCell !== undefined,
      default: defaultValue,
    };
  }

  if (data.operator === 'val') {
    const pathCells = data.cells.filter((c) => c.fieldId === 'path');
    const scopeCell = data.cells.find((c) => c.fieldId === 'scopeLevel');
    const scope = typeof scopeCell?.value === 'number' ? scopeCell.value : 0;
    const components = pathCells.flatMap((c) => pathComponentsFromCellValue(c.value));

    if (isMetadataAccess(scope, components)) {
      return {
        accessType: 'metadata',
        metadataKey: components[0],
      };
    }

    return {
      accessType: 'path',
      scopeLevel: scope,
      path: components,
    };
  }

  if (data.operator === 'exists') {
    const pathCell = data.cells.find((c) => c.fieldId === 'path');
    const pathValue = pathCell?.value;
    const isArrayPath = Array.isArray(pathValue);
    return {
      pathType: isArrayPath ? 'array' : 'dot',
      dotPath: isArrayPath ? '' : String(pathValue ?? ''),
      arrayPath: isArrayPath ? (pathValue as unknown[]).map(String) : [],
    };
  }

  return {};
}

function getLiteralInitialValues(data: LiteralNodeData): Record<string, unknown> {
  return {
    valueType: data.valueType,
    value: data.value,
  };
}

function getStructureInitialValues(data: StructureNodeData): Record<string, unknown> {
  return {
    valueType: data.isArray ? 'array' : 'object',
    mode: 'template',
  };
}

/**
 * Get a display label for a node
 */
export function getNodeDisplayLabel(data: LogicNodeData): string {
  switch (data.type) {
    case 'operator':
      return data.label || data.operator.toUpperCase();
    case 'literal':
      return 'LITERAL';
    case 'structure':
      return data.isArray ? 'ARRAY' : 'OBJECT';
    default:
      return 'NODE';
  }
}

/**
 * Get the category for a node
 */
export function getNodeCategory(data: LogicNodeData): string | null {
  switch (data.type) {
    case 'operator':
      return data.category ? capitalizeFirst(data.category) : null;
    case 'literal':
      return 'Literal';
    case 'structure':
      return 'Structure';
    default:
      return null;
  }
}

function capitalizeFirst(str: string): string {
  return str.charAt(0).toUpperCase() + str.slice(1);
}
