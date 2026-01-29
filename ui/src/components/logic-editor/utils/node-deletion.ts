/**
 * Node Deletion Utilities
 *
 * Functions for deleting nodes while maintaining tree consistency.
 */

import type {
  LogicNode,
  OperatorNodeData,
  VerticalCellNodeData,
  DecisionNodeData,
  StructureNodeData,
  JsonLogicValue,
} from '../types';
import { rebuildOperatorExpression } from './expression-builder';

/**
 * Delete a node and all its descendants from the node array.
 * Also updates parent references to remove the deleted node.
 *
 * @param nodeId The ID of the node to delete
 * @param nodes The current array of nodes
 * @returns A new array with the node and its descendants removed
 */
export function deleteNodeAndDescendants(
  nodeId: string,
  nodes: LogicNode[]
): LogicNode[] {
  // Get all descendant IDs
  const idsToDelete = getDescendantIds(nodeId, nodes);
  idsToDelete.add(nodeId);

  // Find the node being deleted to get its parent info
  const deletedNode = nodes.find((n) => n.id === nodeId);
  const parentId = deletedNode?.data.parentId;

  // Filter out deleted nodes
  const filteredNodes = nodes.filter((node) => !idsToDelete.has(node.id));

  // Update parent references and rebuild expression
  return filteredNodes.map((node) => {
    // Update parent nodes that reference the deleted node
    if (parentId && node.id === parentId) {
      return updateParentAfterChildDeletion(node, nodeId, filteredNodes);
    }
    return node;
  });
}

/**
 * Get all descendant node IDs for a given node.
 */
export function getDescendantIds(nodeId: string, nodes: LogicNode[]): Set<string> {
  const descendants = new Set<string>();
  const node = nodes.find((n) => n.id === nodeId);

  if (!node) return descendants;

  // Get direct child IDs based on node type
  const childIds = getChildIds(node.data);

  // Recursively collect all descendants
  for (const childId of childIds) {
    descendants.add(childId);
    const childDescendants = getDescendantIds(childId, nodes);
    for (const id of childDescendants) {
      descendants.add(id);
    }
  }

  return descendants;
}

/**
 * Get direct child IDs from node data
 */
function getChildIds(data: LogicNode['data']): string[] {
  switch (data.type) {
    case 'operator': {
      const opData = data as OperatorNodeData;
      return opData.childIds || [];
    }
    case 'verticalCell': {
      const vcData = data as VerticalCellNodeData;
      const ids: string[] = [];
      for (const cell of vcData.cells) {
        if (cell.branchId) ids.push(cell.branchId);
        if (cell.conditionBranchId) ids.push(cell.conditionBranchId);
        if (cell.thenBranchId) ids.push(cell.thenBranchId);
      }
      return ids;
    }
    case 'decision': {
      const decData = data as DecisionNodeData;
      const ids: string[] = [];
      if (decData.conditionBranchId) ids.push(decData.conditionBranchId);
      ids.push(decData.yesBranchId);
      ids.push(decData.noBranchId);
      return ids;
    }
    case 'structure': {
      const structData = data as StructureNodeData;
      return structData.elements
        .filter((el) => el.branchId)
        .map((el) => el.branchId!);
    }
    case 'literal':
    case 'variable':
    default:
      return [];
  }
}

/**
 * Update a parent node after one of its children is deleted.
 * Creates fully immutable updates - no mutation of original objects.
 * Also rebuilds the expression from remaining children.
 */
function updateParentAfterChildDeletion(
  parentNode: LogicNode,
  deletedChildId: string,
  allNodes: LogicNode[]
): LogicNode {
  const data = parentNode.data;

  switch (data.type) {
    case 'operator': {
      const opData = data as OperatorNodeData;
      const newChildIds = opData.childIds.filter((id) => id !== deletedChildId);

      // Get remaining child nodes and reindex them
      const remainingChildren = allNodes
        .filter((n) => newChildIds.includes(n.id))
        .sort((a, b) => (a.data.argIndex ?? 0) - (b.data.argIndex ?? 0))
        .map((n, idx) => ({
          ...n,
          data: { ...n.data, argIndex: idx },
        }));

      // Rebuild expression from remaining children
      const newExpression = rebuildOperatorExpression(opData.operator, remainingChildren);

      return {
        ...parentNode,
        data: {
          ...opData,
          childIds: newChildIds,
          expression: newExpression,
          expressionText: undefined, // Clear cached text
        },
      };
    }
    case 'verticalCell': {
      const vcData = data as VerticalCellNodeData;

      // Filter and reindex cells
      const newCells = vcData.cells
        .filter((cell) => cell.branchId !== deletedChildId)
        .map((cell, idx) => ({
          ...cell,
          index: idx,
          conditionBranchId:
            cell.conditionBranchId === deletedChildId
              ? undefined
              : cell.conditionBranchId,
          thenBranchId:
            cell.thenBranchId === deletedChildId
              ? undefined
              : cell.thenBranchId,
        }));

      // Rebuild expression from remaining cells
      const newOperands = newCells.map((cell) => {
        if (cell.branchId) {
          const branchNode = allNodes.find((n) => n.id === cell.branchId);
          if (branchNode) {
            return branchNode.data.expression;
          }
        }
        // Fallback to stored expression value at this index if available
        const storedExpr = vcData.expression;
        if (storedExpr && typeof storedExpr === 'object' && !Array.isArray(storedExpr)) {
          const opKey = Object.keys(storedExpr)[0];
          const operands = (storedExpr as Record<string, unknown>)[opKey];
          if (Array.isArray(operands) && cell.index < operands.length) {
            return operands[cell.index];
          }
        }
        return null;
      });

      const newExpression = { [vcData.operator]: newOperands };

      return {
        ...parentNode,
        data: {
          ...vcData,
          cells: newCells,
          expression: newExpression,
          expressionText: undefined,
        },
      };
    }
    case 'decision': {
      const decData = data as DecisionNodeData;
      // For decision nodes, we can't really remove branches - they're required
      // Instead, we clear the reference if it matches
      return {
        ...parentNode,
        data: {
          ...decData,
          conditionBranchId:
            decData.conditionBranchId === deletedChildId
              ? undefined
              : decData.conditionBranchId,
          isConditionComplex:
            decData.conditionBranchId === deletedChildId
              ? false
              : decData.isConditionComplex,
        },
      };
    }
    case 'structure': {
      const structData = data as StructureNodeData;
      const newElements = structData.elements.filter(
        (el) => el.branchId !== deletedChildId
      );

      // Rebuild expression from remaining elements
      let newExpression: JsonLogicValue;
      if (structData.isArray) {
        newExpression = newElements.map((el) => {
          if (el.type === 'inline') {
            return el.value ?? null;
          } else if (el.branchId) {
            const branchNode = allNodes.find((n) => n.id === el.branchId);
            return branchNode?.data.expression ?? null;
          }
          return null;
        });
      } else {
        const obj: Record<string, JsonLogicValue> = {};
        for (const el of newElements) {
          if (el.key) {
            if (el.type === 'inline') {
              obj[el.key] = (el.value ?? null) as JsonLogicValue;
            } else if (el.branchId) {
              const branchNode = allNodes.find((n) => n.id === el.branchId);
              obj[el.key] = branchNode?.data.expression ?? null;
            }
          }
        }
        newExpression = obj;
      }

      return {
        ...parentNode,
        data: {
          ...structData,
          elements: newElements,
          expression: newExpression,
          expressionText: undefined,
        },
      };
    }
    default:
      return parentNode;
  }
}

/**
 * Check if a node is the root node (has no parent)
 */
export function isRootNode(node: LogicNode): boolean {
  return !node.data.parentId;
}

/**
 * Check if a node can be deleted.
 * Root nodes cannot be deleted.
 */
export function canDeleteNode(node: LogicNode): boolean {
  return !isRootNode(node);
}
