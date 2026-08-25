/**
 * Node Cloning Utility
 *
 * Provides shared logic for cloning nodes with ID remapping.
 * Used by pasteNode, duplicateNode, and other operations that
 * need to create copies of node trees with new unique IDs.
 */

import { v4 as uuidv4 } from 'uuid';
import type { LogicNode, OperatorNodeData, StructureNodeData } from '../types';

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
 * - Remapping cell branch IDs for operator nodes
 * - Remapping element branch IDs for structure (template) nodes
 *
 * @param nodes - The nodes to clone (should include the root and all descendants)
 * @param rootId - The ID of the root node in the original set
 * @returns CloneResult with the cloned nodes, ID mapping, and new root ID
 */
export function cloneNodesWithIdMapping(
  nodes: LogicNode[],
  rootId: string
): CloneResult {
  // Validate that rootId exists in the nodes array
  const rootExists = nodes.some((n) => n.id === rootId);
  if (!rootExists) {
    console.warn('cloneNodesWithIdMapping: rootId not found in nodes array');
    // Return empty result to allow caller to handle gracefully
    return {
      nodes: [],
      idMap: new Map(),
      newRootId: '',
    };
  }

  // Create ID mapping for all nodes
  const idMap = new Map<string, string>();
  nodes.forEach((n) => {
    idMap.set(n.id, uuidv4());
  });

  const remap = (id: string | undefined): string | undefined =>
    id && idMap.has(id) ? idMap.get(id) : id;

  // Clone and remap IDs
  const clonedNodes: LogicNode[] = nodes.map((n) => {
    const newId = idMap.get(n.id)!;
    const newNode: LogicNode = {
      ...JSON.parse(JSON.stringify(n)), // Deep clone
      id: newId,
      data: {
        ...JSON.parse(JSON.stringify(n.data)),
        // Remap parentId if it's in the cloned set
        parentId: remap(n.data.parentId),
      },
    };

    // Remap cells for operator nodes
    if (newNode.data.type === 'operator') {
      const opData = newNode.data as OperatorNodeData;
      newNode.data = {
        ...opData,
        cells: opData.cells.map((cell) => ({
          ...cell,
          branchId: remap(cell.branchId),
          conditionBranchId: remap(cell.conditionBranchId),
          thenBranchId: remap(cell.thenBranchId),
        })),
      };
    }

    // Remap elements for structure (template) nodes
    if (newNode.data.type === 'structure') {
      const structData = newNode.data as StructureNodeData;
      newNode.data = {
        ...structData,
        elements: structData.elements.map((element) => ({
          ...element,
          branchId: remap(element.branchId),
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
 * handling operator nodes (via cells array).
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

    if (currentNode?.data.type === 'operator') {
      const opData = currentNode.data as OperatorNodeData;
      for (const cell of opData.cells) {
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
 * Return a copy of `node` whose references to child `oldChildId` point at
 * `newChildId` instead: operator cells (branch / condition / then) and
 * structure elements alike. Nodes that do not reference the child are
 * returned unchanged (same object).
 */
export function replaceChildReference(
  node: LogicNode,
  oldChildId: string,
  newChildId: string
): LogicNode {
  if (node.data.type === 'operator') {
    const opData = node.data as OperatorNodeData;
    if (
      !opData.cells.some(
        (cell) =>
          cell.branchId === oldChildId ||
          cell.conditionBranchId === oldChildId ||
          cell.thenBranchId === oldChildId
      )
    ) {
      return node;
    }
    return {
      ...node,
      data: {
        ...opData,
        cells: opData.cells.map((cell) => ({
          ...cell,
          branchId: cell.branchId === oldChildId ? newChildId : cell.branchId,
          conditionBranchId: cell.conditionBranchId === oldChildId ? newChildId : cell.conditionBranchId,
          thenBranchId: cell.thenBranchId === oldChildId ? newChildId : cell.thenBranchId,
        })),
      },
    };
  }

  if (node.data.type === 'structure') {
    const structData = node.data as StructureNodeData;
    if (!structData.elements.some((el) => el.branchId === oldChildId)) {
      return node;
    }
    return {
      ...node,
      data: {
        ...structData,
        elements: structData.elements.map((el) => ({
          ...el,
          branchId: el.branchId === oldChildId ? newChildId : el.branchId,
        })),
      },
    };
  }

  return node;
}

/**
 * Update parent references when replacing a node in the tree.
 *
 * When a node is replaced (e.g., during paste), the parent's
 * cells (or structure elements) need to be updated to point to the new node ID.
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
    return replaceChildReference(n, oldChildId, newChildId);
  });
}
