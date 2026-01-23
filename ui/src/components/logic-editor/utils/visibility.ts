import type { LogicNode, VerticalCellNodeData } from '../types';
import { isOperatorNode, isVerticalCellNode } from './type-guards';

// Helper to collect all branch IDs from a cell
function collectCellBranchIds(cell: VerticalCellNodeData['cells'][0], target: Set<string>): void {
  if (cell.branchId) target.add(cell.branchId);
  if (cell.conditionBranchId) target.add(cell.conditionBranchId);
  if (cell.thenBranchId) target.add(cell.thenBranchId);
}

// Build a parent-child map for O(1) child lookups
function buildParentChildMap(nodes: LogicNode[]): Map<string, string[]> {
  const parentChildMap = new Map<string, string[]>();

  nodes.forEach((node) => {
    const parentId = node.data.parentId;
    if (parentId) {
      const children = parentChildMap.get(parentId) || [];
      children.push(node.id);
      parentChildMap.set(parentId, children);
    }
  });

  return parentChildMap;
}

// Get all hidden node IDs (descendants of collapsed nodes or collapsed cells)
export function getHiddenNodeIds(nodes: LogicNode[]): Set<string> {
  const hiddenIds = new Set<string>();
  const collapsedNodeIds = new Set<string>();
  const collapsedBranchIds = new Set<string>();

  // Build parent-child map for efficient lookups
  const parentChildMap = buildParentChildMap(nodes);

  // Find all collapsed operator nodes and collapsed cells in vertical cell nodes
  nodes.forEach((node) => {
    if (isOperatorNode(node)) {
      if (node.data.collapsed) {
        collapsedNodeIds.add(node.id);
      }
    } else if (isVerticalCellNode(node)) {
      const vcData = node.data;

      // If the entire node is collapsed, hide all its branch children
      if (vcData.collapsed) {
        vcData.cells.forEach((cell) => {
          collectCellBranchIds(cell, collapsedBranchIds);
        });
      } else {
        // Otherwise, only hide individually collapsed cells
        const collapsedIndices = vcData.collapsedCellIndices || [];
        vcData.cells.forEach((cell) => {
          if (collapsedIndices.includes(cell.index)) {
            collectCellBranchIds(cell, collapsedBranchIds);
          }
        });
      }
    }
  });

  // Recursively mark children of a node as hidden using the map
  function markDescendantsHidden(parentId: string): void {
    const children = parentChildMap.get(parentId);
    if (!children) return;

    for (const childId of children) {
      if (!hiddenIds.has(childId)) {
        hiddenIds.add(childId);
        // Also mark this node's children as hidden
        markDescendantsHidden(childId);
      }
    }
  }

  // Mark descendants of all collapsed nodes
  collapsedNodeIds.forEach((collapsedId) => {
    markDescendantsHidden(collapsedId);
  });

  // Mark collapsed branch nodes and their descendants as hidden
  collapsedBranchIds.forEach((branchId) => {
    hiddenIds.add(branchId);
    markDescendantsHidden(branchId);
  });

  return hiddenIds;
}
