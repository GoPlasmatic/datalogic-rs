/**
 * Properties Panel Utilities
 *
 * Helper functions for mapping node data to panel configurations.
 */

import type { LogicNodeData, VariableNodeData, LiteralNodeData, OperatorNodeData, VerticalCellNodeData, StructureNodeData } from '../types';
import type { Operator, PanelConfig } from '../config/operators.types';
import { getOperator } from '../config/operators';
import { literalPanelConfig } from '../config/literalPanel';

/**
 * Get the panel configuration for a node
 */
export function getPanelConfigForNode(data: LogicNodeData): PanelConfig | null {
  switch (data.type) {
    case 'variable':
      return getVariablePanelConfig(data);
    case 'literal':
      return literalPanelConfig;
    case 'operator':
      return getOperatorPanelConfig(data);
    case 'verticalCell':
      return getVerticalCellPanelConfig(data);
    case 'decision':
      return getDecisionPanelConfig();
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
    case 'variable':
      return getOperator(data.operator) ?? null;
    case 'operator':
      return getOperator(data.operator) ?? null;
    case 'verticalCell':
      return getOperator(data.operator) ?? null;
    case 'decision':
      return getOperator('if') ?? null;
    default:
      return null;
  }
}

function getVariablePanelConfig(data: VariableNodeData): PanelConfig | null {
  const op = getOperator(data.operator);
  return op?.panel ?? null;
}

function getOperatorPanelConfig(data: OperatorNodeData): PanelConfig | null {
  const op = getOperator(data.operator);
  return op?.panel ?? null;
}

function getVerticalCellPanelConfig(data: VerticalCellNodeData): PanelConfig | null {
  const op = getOperator(data.operator);
  return op?.panel ?? null;
}

function getDecisionPanelConfig(): PanelConfig | null {
  const op = getOperator('if');
  return op?.panel ?? null;
}

function getStructurePanelConfig(): PanelConfig | null {
  // Structure nodes use the literal panel config
  return literalPanelConfig;
}

/**
 * Extract initial panel values from node data
 */
export function getInitialValuesFromNode(data: LogicNodeData): Record<string, unknown> {
  switch (data.type) {
    case 'variable':
      return getVariableInitialValues(data);
    case 'literal':
      return getLiteralInitialValues(data);
    case 'operator':
      return {}; // Operator arguments are handled via connections
    case 'verticalCell':
      return {}; // Cell contents are handled via connections
    case 'decision':
      return {}; // Decision branches are handled via connections
    case 'structure':
      return getStructureInitialValues(data);
    default:
      return {};
  }
}

function getVariableInitialValues(data: VariableNodeData): Record<string, unknown> {
  if (data.operator === 'var') {
    return {
      path: data.path,
      hasDefault: data.defaultValue !== undefined,
      default: data.defaultValue,
    };
  }

  if (data.operator === 'val') {
    // Check if it's accessing metadata (index/key)
    if (data.pathComponents?.length === 1 &&
        (data.pathComponents[0] === 'index' || data.pathComponents[0] === 'key')) {
      return {
        accessType: 'metadata',
        metadataKey: data.pathComponents[0],
      };
    }
    // Also check legacy path format
    if (data.path === 'index' || data.path === 'key') {
      return {
        accessType: 'metadata',
        metadataKey: data.path,
      };
    }
    return {
      accessType: 'path',
      scopeLevel: data.scopeJump ?? 0,
      path: data.pathComponents ?? (data.path ? data.path.split('.') : []),
    };
  }

  if (data.operator === 'exists') {
    // Determine if it's dot notation or array path
    const isDotNotation = typeof data.path === 'string' && !data.path.startsWith('[');
    return {
      pathType: isDotNotation ? 'dot' : 'array',
      dotPath: isDotNotation ? data.path : '',
      arrayPath: isDotNotation ? [] : (data.path ? data.path.split('.') : []),
    };
  }

  return { path: data.path };
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
    mode: 'template', // Structures with expressions are templates
  };
}

/**
 * Get a display label for a node
 */
export function getNodeDisplayLabel(data: LogicNodeData): string {
  switch (data.type) {
    case 'variable':
      return data.operator.toUpperCase();
    case 'literal':
      return 'LITERAL';
    case 'operator':
      return data.label || data.operator.toUpperCase();
    case 'verticalCell':
      return data.label || data.operator.toUpperCase();
    case 'decision':
      return 'IF';
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
    case 'variable':
      return 'Variable';
    case 'literal':
      return 'Literal';
    case 'operator':
    case 'verticalCell':
      return data.category ? capitalizeFirst(data.category) : null;
    case 'decision':
      return 'Control';
    case 'structure':
      return 'Structure';
    default:
      return null;
  }
}

function capitalizeFirst(str: string): string {
  return str.charAt(0).toUpperCase() + str.slice(1);
}
