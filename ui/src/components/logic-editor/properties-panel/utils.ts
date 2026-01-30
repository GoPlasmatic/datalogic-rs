/**
 * Properties Panel Utilities
 *
 * Helper functions for mapping node data to panel configurations.
 */

import type { LogicNodeData, LiteralNodeData, OperatorNodeData, StructureNodeData } from '../types';
import type { Operator, PanelConfig } from '../config/operators.types';
import { getOperator } from '../config/operators';
import { literalPanelConfig } from '../config/literalPanel';

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
  return literalPanelConfig;
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
  // For variable operators, extract values from editable cells
  if (data.operator === 'var') {
    const pathCell = data.cells.find((c) => c.fieldId === 'path');
    const defaultCell = data.cells.find((c) => c.fieldId === 'default');
    return {
      path: pathCell?.value ?? '',
      hasDefault: defaultCell !== undefined,
      default: defaultCell?.value,
    };
  }

  if (data.operator === 'val') {
    const pathCell = data.cells.find((c) => c.fieldId === 'path');
    const scopeCell = data.cells.find((c) => c.fieldId === 'scopeLevel');
    const metaCell = data.cells.find((c) => c.fieldId === 'metadataKey');

    if (metaCell) {
      return {
        accessType: 'metadata',
        metadataKey: metaCell.value,
      };
    }

    return {
      accessType: 'path',
      scopeLevel: scopeCell?.value ?? 0,
      path: pathCell?.value ?? [],
    };
  }

  if (data.operator === 'exists') {
    const pathCell = data.cells.find((c) => c.fieldId === 'path');
    const pathValue = pathCell?.value as string | undefined;
    const isDotNotation = typeof pathValue === 'string' && !pathValue.startsWith('[');
    return {
      pathType: isDotNotation ? 'dot' : 'array',
      dotPath: isDotNotation ? (pathValue ?? '') : '',
      arrayPath: isDotNotation ? [] : (pathValue ? String(pathValue).split('.') : []),
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
