/**
 * Node Transform Service
 *
 * Provides pure functions for transforming nodes (wrapping, duplicating).
 */

import { v4 as uuidv4 } from 'uuid';
import type {
  LogicNode,
  OperatorNodeData,
} from '../types';
import { getOperator } from '../config/operators';
import { cloneNodesWithIdMapping, getDescendants, updateParentChildReference } from '../utils/node-cloning';

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
      icon: 'list',
      cells: [{
        type: 'branch',
        branchId: nodeId,
        index: 0,
      }],
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
    // Update parent's cells to point to the new wrapper
    if (n.id === targetNode.data.parentId && n.data.type === 'operator') {
      const opData = n.data as OperatorNodeData;
      return {
        ...n,
        data: {
          ...opData,
          cells: opData.cells.map((cell) => ({
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
      const newArgIndex = opData.cells.length;

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
              cells: [...opData.cells, {
                type: 'branch' as const,
                branchId: newRootId,
                index: newArgIndex,
              }],
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

// Re-export cloning utilities used elsewhere
export { cloneNodesWithIdMapping, getDescendants, updateParentChildReference };
