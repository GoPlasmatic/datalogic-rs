/**
 * Node Mutation Service
 *
 * Provides pure functions for node mutations.
 * The actual state management and side effects are handled by EditorContext.
 * This module contains the logic for manipulating nodes without React state concerns.
 */

import { v4 as uuidv4 } from 'uuid';
import type {
  LogicNode,
  OperatorNodeData,
  LiteralNodeData,
  VerticalCellNodeData,
  VariableNodeData,
  JsonLogicValue,
} from '../types';
import { getOperator } from '../config/operators';
import { deleteNodeAndDescendants } from '../utils/node-deletion';
import { rebuildOperatorExpression } from '../utils/expression-builder';
import { cloneNodesWithIdMapping, getDescendants, updateParentChildReference } from '../utils/node-cloning';

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
      type: 'variable',
      position: { x: 0, y: 0 },
      data: {
        type: 'variable',
        operator: 'var',
        path: '',
        expression: { var: '' },
        parentId,
        argIndex,
      } as VariableNodeData,
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
        childIds: [childId],
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

/**
 * Result of adding an argument
 */
export interface AddArgumentResult {
  nodes: LogicNode[];
  newNodeId: string;
}

/**
 * Add an argument to an operator node
 */
export function addArgument(
  nodes: LogicNode[],
  parentId: string,
  nodeType: 'literal' | 'variable' | 'operator',
  operatorName?: string
): AddArgumentResult | null {
  const parentNode = nodes.find((n) => n.id === parentId);
  if (!parentNode) return null;

  const parentData = parentNode.data;

  if (parentData.type === 'operator') {
    const operatorData = parentData as OperatorNodeData;
    const opConfig = getOperator(operatorData.operator);

    if (
      !opConfig ||
      (opConfig.arity.type !== 'nary' &&
        opConfig.arity.type !== 'variadic' &&
        opConfig.arity.type !== 'chainable')
    ) {
      return null;
    }

    if (opConfig.arity.max && operatorData.childIds.length >= opConfig.arity.max) {
      return null;
    }

    const expr = operatorData.expression;
    let currentOperands: unknown[] = [];
    if (expr && typeof expr === 'object' && !Array.isArray(expr)) {
      const opKey = Object.keys(expr)[0];
      const operands = (expr as Record<string, unknown>)[opKey];
      currentOperands = Array.isArray(operands) ? operands : [operands];
    }

    const newArgIndex = currentOperands.length;
    const newNodes = createArgumentNode(nodeType, parentId, newArgIndex, opConfig.category, operatorName);
    const newNodeId = newNodes[0].id;

    const newNodeData = newNodes[0].data;
    let newValue: unknown = 0;
    if (newNodeData.type === 'literal') {
      newValue = (newNodeData as LiteralNodeData).value;
    } else if (newNodeData.type === 'variable') {
      newValue = (newNodeData as VariableNodeData).expression;
    } else if (newNodeData.type === 'operator') {
      newValue = (newNodeData as OperatorNodeData).expression;
    }

    const opKey = expr && typeof expr === 'object' && !Array.isArray(expr)
      ? Object.keys(expr)[0]
      : operatorData.operator;
    const newOperands = [...currentOperands, newValue] as JsonLogicValue[];

    const updatedParent: LogicNode = {
      ...parentNode,
      data: {
        ...operatorData,
        childIds: [...operatorData.childIds, newNodeId],
        expression: { [opKey]: newOperands } as JsonLogicValue,
        expressionText: undefined,
      },
    };

    const result = nodes.map((n) => (n.id === parentId ? updatedParent : n));
    result.push(...newNodes);

    return { nodes: result, newNodeId };
  }

  if (parentData.type === 'verticalCell') {
    const verticalData = parentData as VerticalCellNodeData;
    const opConfig = getOperator(verticalData.operator);

    if (
      !opConfig ||
      (opConfig.arity.type !== 'nary' &&
        opConfig.arity.type !== 'variadic' &&
        opConfig.arity.type !== 'chainable')
    ) {
      return null;
    }

    if (opConfig.arity.max && verticalData.cells.length >= opConfig.arity.max) {
      return null;
    }

    const expr = verticalData.expression;
    let currentOperands: JsonLogicValue[] = [];
    if (expr && typeof expr === 'object' && !Array.isArray(expr)) {
      const opKey = Object.keys(expr)[0];
      const operands = (expr as Record<string, unknown>)[opKey];
      currentOperands = Array.isArray(operands) ? operands : [operands as JsonLogicValue];
    }

    const newIndex = currentOperands.length;
    const newNodes = createArgumentNode(nodeType, parentId, newIndex, opConfig.category, operatorName);
    const newNodeId = newNodes[0].id;

    const newNodeData = newNodes[0].data;
    let newValue: JsonLogicValue = 0;
    if (newNodeData.type === 'literal') {
      newValue = (newNodeData as LiteralNodeData).value as JsonLogicValue;
    } else if (newNodeData.type === 'variable') {
      newValue = (newNodeData as VariableNodeData).expression as JsonLogicValue;
    } else if (newNodeData.type === 'operator') {
      newValue = (newNodeData as OperatorNodeData).expression as JsonLogicValue;
    }

    const newOperands = [...currentOperands, newValue];
    const newExpression = { [verticalData.operator]: newOperands } as JsonLogicValue;

    const updatedParent: LogicNode = {
      ...parentNode,
      data: {
        ...verticalData,
        cells: [
          ...verticalData.cells,
          {
            type: 'branch' as const,
            branchId: newNodeId,
            index: newIndex,
          },
        ],
        expression: newExpression,
        expressionText: undefined,
      },
    };

    const result = nodes.map((n) => (n.id === parentId ? updatedParent : n));
    result.push(...newNodes);

    return { nodes: result, newNodeId };
  }

  return null;
}

/**
 * Remove an argument from an operator node
 */
export function removeArgument(
  nodes: LogicNode[],
  parentId: string,
  argIndex: number
): LogicNode[] | null {
  const parentNode = nodes.find((n) => n.id === parentId);
  if (!parentNode) return null;

  const parentData = parentNode.data;

  if (parentData.type === 'operator') {
    const operatorData = parentData as OperatorNodeData;
    const opConfig = getOperator(operatorData.operator);

    const minArgs = opConfig?.arity.min ?? 0;
    if (operatorData.childIds.length <= minArgs) {
      return null;
    }

    const childToRemove = nodes.find(
      (n) => n.data.parentId === parentId && n.data.argIndex === argIndex
    );
    if (!childToRemove) return null;

    let newNodes = deleteNodeAndDescendants(childToRemove.id, nodes);
    const newChildIds = operatorData.childIds.filter((id) => id !== childToRemove.id);

    // Reindex remaining children
    newNodes = newNodes.map((n) => {
      if (n.data.parentId === parentId && (n.data.argIndex ?? 0) > argIndex) {
        return {
          ...n,
          data: { ...n.data, argIndex: (n.data.argIndex ?? 0) - 1 },
        };
      }
      return n;
    });

    const remainingChildren = newNodes.filter((n) => newChildIds.includes(n.id));
    const newExpression = rebuildOperatorExpression(operatorData.operator, remainingChildren);

    newNodes = newNodes.map((n) => {
      if (n.id === parentId) {
        return {
          ...n,
          data: {
            ...operatorData,
            childIds: newChildIds,
            expression: newExpression,
            expressionText: undefined,
          },
        };
      }
      return n;
    });

    return newNodes;
  }

  if (parentData.type === 'verticalCell') {
    const verticalData = parentData as VerticalCellNodeData;
    const opConfig = getOperator(verticalData.operator);

    const minArgs = opConfig?.arity.min ?? 0;
    if (verticalData.cells.length <= minArgs) {
      return null;
    }

    const cellToRemove = verticalData.cells.find((c) => c.index === argIndex);
    if (!cellToRemove) return null;

    const expr = verticalData.expression;
    let currentOperands: JsonLogicValue[] = [];
    if (expr && typeof expr === 'object' && !Array.isArray(expr)) {
      const opKey = Object.keys(expr)[0];
      const operands = (expr as Record<string, unknown>)[opKey];
      currentOperands = Array.isArray(operands) ? operands : [operands as JsonLogicValue];
    }

    let newNodes = cellToRemove.branchId
      ? deleteNodeAndDescendants(cellToRemove.branchId, nodes)
      : nodes;

    const newOperands = currentOperands.filter((_, i) => i !== argIndex);
    const newExpression = { [verticalData.operator]: newOperands } as JsonLogicValue;

    newNodes = newNodes.map((n) => {
      if (n.id === parentId) {
        const updatedCells = verticalData.cells
          .filter((c) => c.index !== argIndex)
          .map((c) => ({
            ...c,
            index: c.index > argIndex ? c.index - 1 : c.index,
          }));
        return {
          ...n,
          data: {
            ...verticalData,
            cells: updatedCells,
            expression: newExpression,
            expressionText: undefined,
          },
        };
      }
      if (n.data.parentId === parentId && (n.data.argIndex ?? 0) > argIndex) {
        return {
          ...n,
          data: {
            ...n.data,
            argIndex: (n.data.argIndex ?? 0) - 1,
          },
        };
      }
      return n;
    });

    return newNodes;
  }

  return null;
}

/**
 * Wrap a node in an operator
 */
export function wrapInOperator(
  nodes: LogicNode[],
  nodeId: string,
  operator: string
): LogicNode[] | null {
  const targetNode = nodes.find((n) => n.id === nodeId);
  if (!targetNode) return null;

  const newOperatorId = uuidv4();
  const opConfig = getOperator(operator);

  const wrapperNode: LogicNode = {
    id: newOperatorId,
    type: 'operator',
    position: { x: 0, y: 0 },
    data: {
      type: 'operator',
      operator,
      category: opConfig?.category || 'logical',
      label: opConfig?.label || operator,
      childIds: [nodeId],
      expression: { [operator]: [] },
      parentId: targetNode.data.parentId,
      argIndex: targetNode.data.argIndex,
    } as OperatorNodeData,
  };

  const updatedTarget: LogicNode = {
    ...targetNode,
    data: {
      ...targetNode.data,
      parentId: newOperatorId,
      argIndex: 0,
    },
  };

  let updatedNodes = nodes.map((n) => {
    if (n.id === nodeId) {
      return updatedTarget;
    }
    // Update parent's childIds
    if (n.id === targetNode.data.parentId && n.data.type === 'operator') {
      const opData = n.data as OperatorNodeData;
      return {
        ...n,
        data: {
          ...opData,
          childIds: opData.childIds.map((id) => (id === nodeId ? newOperatorId : id)),
        },
      };
    }
    // Update parent's cells if it's a verticalCell
    if (n.id === targetNode.data.parentId && n.data.type === 'verticalCell') {
      const vcData = n.data as VerticalCellNodeData;
      return {
        ...n,
        data: {
          ...vcData,
          cells: vcData.cells.map((cell) => ({
            ...cell,
            branchId: cell.branchId === nodeId ? newOperatorId : cell.branchId,
            conditionBranchId: cell.conditionBranchId === nodeId ? newOperatorId : cell.conditionBranchId,
            thenBranchId: cell.thenBranchId === nodeId ? newOperatorId : cell.thenBranchId,
          })),
        },
      };
    }
    return n;
  });

  updatedNodes = [...updatedNodes, wrapperNode];

  return updatedNodes;
}

/**
 * Duplicate a node and its descendants
 */
export function duplicateNodeTree(
  nodes: LogicNode[],
  nodeId: string
): { nodes: LogicNode[]; newRootId: string } | null {
  const targetNode = nodes.find((n) => n.id === nodeId);
  if (!targetNode) return null;

  const descendants = getDescendants(nodeId, nodes);
  const nodesToClone = [targetNode, ...descendants];

  const { nodes: clonedNodes, newRootId } = cloneNodesWithIdMapping(nodesToClone, nodeId);
  const clonedRoot = clonedNodes.find((n) => n.id === newRootId)!;

  // If the original had a parent, add as sibling
  if (targetNode.data.parentId) {
    const parent = nodes.find((n) => n.id === targetNode.data.parentId);
    if (parent && parent.data.type === 'operator') {
      const opData = parent.data as OperatorNodeData;
      const newArgIndex = opData.childIds.length;

      clonedRoot.data = {
        ...clonedRoot.data,
        argIndex: newArgIndex,
      };

      const updatedNodes = nodes.map((n) => {
        if (n.id === targetNode.data.parentId) {
          return {
            ...n,
            data: {
              ...opData,
              childIds: [...opData.childIds, newRootId],
            },
          };
        }
        return n;
      });

      return { nodes: [...updatedNodes, ...clonedNodes], newRootId };
    }
  }

  // If no parent or parent isn't operator, replace entire tree
  clonedRoot.data = {
    ...clonedRoot.data,
    parentId: undefined,
    argIndex: undefined,
  };

  return { nodes: clonedNodes, newRootId };
}

// Re-export utilities that are used in EditorContext
export { cloneNodesWithIdMapping, getDescendants, updateParentChildReference };
