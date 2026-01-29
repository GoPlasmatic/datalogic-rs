/**
 * Node Cloning Utility
 *
 * Provides shared logic for cloning nodes with ID remapping.
 * Used by pasteNode, duplicateNode, and other operations that
 * need to create copies of node trees with new unique IDs.
 */

import { v4 as uuidv4 } from 'uuid';
import type { LogicNode, OperatorNodeData, VerticalCellNodeData } from '../types';

/**
 * Result of cloning nodes with ID remapping
 */
export interface CloneResult {
  /** The cloned nodes with new IDs */
  nodes: LogicNode[];
  /** Map from old ID to new ID */
  idMap: Map<string, string>;
  /** The new ID of the root node */
  newRootId: string;
}

/**
 * Clone a set of nodes with new unique IDs, properly remapping all internal references.
 *
 * This handles:
 * - Generating new UUIDs for each node
 * - Remapping parentId references within the cloned set
 * - Remapping childIds for operator nodes
 * - Remapping cell branch IDs for verticalCell nodes
 *
 * @param nodes - The nodes to clone (should include the root and all descendants)
 * @param rootId - The ID of the root node in the original set
 * @returns CloneResult with the cloned nodes, ID mapping, and new root ID
 */
export function cloneNodesWithIdMapping(
  nodes: LogicNode[],
  rootId: string
): CloneResult {
  // Create ID mapping for all nodes
  const idMap = new Map<string, string>();
  nodes.forEach((n) => {
    idMap.set(n.id, uuidv4());
  });

  // Clone and remap IDs
  const clonedNodes: LogicNode[] = nodes.map((n) => {
    const newId = idMap.get(n.id)!;
    const newNode: LogicNode = {
      ...JSON.parse(JSON.stringify(n)), // Deep clone
      id: newId,
      data: {
        ...JSON.parse(JSON.stringify(n.data)),
        // Remap parentId if it's in the cloned set
        parentId: n.data.parentId && idMap.has(n.data.parentId)
          ? idMap.get(n.data.parentId)
          : n.data.parentId,
      },
    };

    // Remap childIds for operator nodes
    if (newNode.data.type === 'operator') {
      const opData = newNode.data as OperatorNodeData;
      newNode.data = {
        ...opData,
        childIds: opData.childIds.map((id) => idMap.get(id) ?? id),
      };
    }

    // Remap cells for verticalCell nodes
    if (newNode.data.type === 'verticalCell') {
      const vcData = newNode.data as VerticalCellNodeData;
      newNode.data = {
        ...vcData,
        cells: vcData.cells.map((cell) => ({
          ...cell,
          branchId: cell.branchId && idMap.has(cell.branchId)
            ? idMap.get(cell.branchId)
            : cell.branchId,
          conditionBranchId: cell.conditionBranchId && idMap.has(cell.conditionBranchId)
            ? idMap.get(cell.conditionBranchId)
            : cell.conditionBranchId,
          thenBranchId: cell.thenBranchId && idMap.has(cell.thenBranchId)
            ? idMap.get(cell.thenBranchId)
            : cell.thenBranchId,
        })),
      };
    }

    return newNode;
  });

  const newRootId = idMap.get(rootId)!;

  return {
    nodes: clonedNodes,
    idMap,
    newRootId,
  };
}

/**
 * Get all descendants of a node recursively.
 *
 * This traverses the node tree to find all child nodes,
 * handling both operator nodes (via parentId) and
 * verticalCell nodes (via cells array).
 *
 * @param nodeId - The ID of the parent node
 * @param allNodes - All nodes in the tree
 * @returns Array of descendant nodes (not including the parent)
 */
export function getDescendants(
  nodeId: string,
  allNodes: LogicNode[]
): LogicNode[] {
  const descendants: LogicNode[] = [];
  const queue = [nodeId];

  while (queue.length > 0) {
    const currentId = queue.shift()!;
    const currentNode = allNodes.find((n) => n.id === currentId);

    // Get children based on node type
    let childIds: string[] = [];

    if (currentNode?.data.type === 'verticalCell') {
      // For verticalCell nodes, get children from cells array
      const vcData = currentNode.data as VerticalCellNodeData;
      for (const cell of vcData.cells) {
        if (cell.branchId) childIds.push(cell.branchId);
        if (cell.conditionBranchId) childIds.push(cell.conditionBranchId);
        if (cell.thenBranchId) childIds.push(cell.thenBranchId);
      }
    } else {
      // For other nodes, find children by parentId
      childIds = allNodes
        .filter((n) => n.data.parentId === currentId)
        .map((n) => n.id);
    }

    const children = childIds
      .map((id) => allNodes.find((n) => n.id === id))
      .filter((n): n is LogicNode => n !== undefined);

    descendants.push(...children);
    queue.push(...children.map((c) => c.id));
  }

  return descendants;
}

/**
 * Update parent references when replacing a node in the tree.
 *
 * When a node is replaced (e.g., during paste), the parent's
 * childIds or cells array needs to be updated to point to the
 * new node ID.
 *
 * @param nodes - The nodes to update
 * @param parentId - The ID of the parent node to update
 * @param oldChildId - The old child ID to replace
 * @param newChildId - The new child ID
 * @returns Updated nodes array
 */
export function updateParentChildReference(
  nodes: LogicNode[],
  parentId: string,
  oldChildId: string,
  newChildId: string
): LogicNode[] {
  return nodes.map((n) => {
    if (n.id !== parentId) return n;

    // Update childIds for operator nodes
    if (n.data.type === 'operator') {
      const opData = n.data as OperatorNodeData;
      return {
        ...n,
        data: {
          ...opData,
          childIds: opData.childIds.map((id) =>
            id === oldChildId ? newChildId : id
          ),
        },
      };
    }

    // Update cells for verticalCell nodes
    if (n.data.type === 'verticalCell') {
      const vcData = n.data as VerticalCellNodeData;
      return {
        ...n,
        data: {
          ...vcData,
          cells: vcData.cells.map((cell) => ({
            ...cell,
            branchId: cell.branchId === oldChildId ? newChildId : cell.branchId,
            conditionBranchId: cell.conditionBranchId === oldChildId ? newChildId : cell.conditionBranchId,
            thenBranchId: cell.thenBranchId === oldChildId ? newChildId : cell.thenBranchId,
          })),
        },
      };
    }

    return n;
  });
}
