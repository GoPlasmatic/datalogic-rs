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
} from '../types';

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

  // Filter out deleted nodes and update parent references
  return nodes
    .filter((node) => !idsToDelete.has(node.id))
    .map((node) => {
      // Update parent nodes that reference the deleted node
      if (deletedNode?.data.parentId === node.id) {
        return updateParentAfterChildDeletion(node, nodeId);
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
 * Update a parent node after one of its children is deleted
 */
function updateParentAfterChildDeletion(
  parentNode: LogicNode,
  deletedChildId: string
): LogicNode {
  const newData = { ...parentNode.data };

  switch (newData.type) {
    case 'operator': {
      const opData = newData as OperatorNodeData;
      opData.childIds = opData.childIds.filter((id) => id !== deletedChildId);
      break;
    }
    case 'verticalCell': {
      const vcData = newData as VerticalCellNodeData;
      vcData.cells = vcData.cells.filter((cell) => {
        // Remove cell if its main branchId matches
        if (cell.branchId === deletedChildId) return false;
        // For if/then cells, clear the reference but keep the cell structure
        if (cell.conditionBranchId === deletedChildId) {
          cell.conditionBranchId = undefined;
        }
        if (cell.thenBranchId === deletedChildId) {
          cell.thenBranchId = undefined;
        }
        return true;
      });
      break;
    }
    case 'decision': {
      const decData = newData as DecisionNodeData;
      // For decision nodes, we can't really remove branches - they're required
      // Instead, we might need to replace with a default value
      if (decData.conditionBranchId === deletedChildId) {
        decData.conditionBranchId = undefined;
        decData.isConditionComplex = false;
      }
      // yesBranchId and noBranchId can't be deleted - would make the if invalid
      break;
    }
    case 'structure': {
      const structData = newData as StructureNodeData;
      structData.elements = structData.elements.filter(
        (el) => el.branchId !== deletedChildId
      );
      break;
    }
  }

  return {
    ...parentNode,
    data: newData,
  };
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
