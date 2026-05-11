/**
 * Node Creation Service
 *
 * Provides pure functions for creating new nodes.
 */

import { v4 as uuidv4 } from 'uuid';
import type {
  LogicNode,
  OperatorNodeData,
  LiteralNodeData,
} from '../types';
import { getOperator } from '../config/operators';
import { buildVariableCells } from '../utils/node-factory';

/**
 * Get default value based on parent operator category
 */
export function getDefaultValueForCategory(
  category: string
): { value: unknown; valueType: 'number' | 'string' | 'boolean' | 'null' } {
  switch (category) {
    case 'arithmetic':
      return { value: 0, valueType: 'number' };
    case 'logical':
      return { value: true, valueType: 'boolean' };
    case 'string':
      return { value: 'text', valueType: 'string' };
    case 'comparison':
      return { value: 0, valueType: 'number' };
    case 'array':
      return { value: 0, valueType: 'number' };
    default:
      return { value: 0, valueType: 'number' };
  }
}

/**
 * Create a new argument node based on type
 */
export function createArgumentNode(
  nodeType: 'literal' | 'variable' | 'operator',
  parentId: string,
  argIndex: number,
  category: string,
  operatorName?: string
): LogicNode[] {
  const newNodeId = uuidv4();

  if (nodeType === 'variable') {
    return [{
      id: newNodeId,
      type: 'operator',
      position: { x: 0, y: 0 },
      data: {
        type: 'operator',
        operator: 'var',
        category: 'variable',
        label: 'Variable',
        icon: 'box',
        cells: buildVariableCells({ operator: 'var', path: '' }),
        expression: { var: '' },
        parentId,
        argIndex,
      } as OperatorNodeData,
    }];
  }

  if (nodeType === 'operator' && operatorName) {
    const opConfig = getOperator(operatorName);
    const opCategory = opConfig?.category || 'arithmetic';
    const { value, valueType } = getDefaultValueForCategory(opCategory);

    const childId = uuidv4();
    const operatorNode: LogicNode = {
      id: newNodeId,
      type: 'operator',
      position: { x: 0, y: 0 },
      data: {
        type: 'operator',
        operator: operatorName,
        category: opCategory,
        label: opConfig?.label || operatorName,
        icon: 'list',
        cells: [{
          type: 'branch',
          branchId: childId,
          index: 0,
        }],
        expression: { [operatorName]: [value] },
        parentId,
        argIndex,
      } as OperatorNodeData,
    };

    const childNode: LogicNode = {
      id: childId,
      type: 'literal',
      position: { x: 0, y: 0 },
      data: {
        type: 'literal',
        value,
        valueType,
        expression: value,
        parentId: newNodeId,
        argIndex: 0,
      } as LiteralNodeData,
    };

    return [operatorNode, childNode];
  }

  // Default: create a literal node
  const { value, valueType } = getDefaultValueForCategory(category);
  return [{
    id: newNodeId,
    type: 'literal',
    position: { x: 0, y: 0 },
    data: {
      type: 'literal',
      value,
      valueType,
      expression: value,
      parentId,
      argIndex,
    } as LiteralNodeData,
  }];
}
