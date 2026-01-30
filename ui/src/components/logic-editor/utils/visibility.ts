import type { LogicNode, OperatorNodeData, StructureNodeData } from '../types';
import { isOperatorNode, isStructureNode } from './type-guards';

// Helper to collect all branch IDs from a cell
function collectCellBranchIds(cell: OperatorNodeData['cells'][0], target: Set<string>): void {
  if (cell.branchId) target.add(cell.branchId);
  if (cell.conditionBranchId) target.add(cell.conditionBranchId);
  if (cell.thenBranchId) target.add(cell.thenBranchId);
}

// Helper to collect all branch IDs from structure elements
function collectStructureBranchIds(elements: StructureNodeData['elements'], target: Set<string>): void {
  elements.forEach((element) => {
    if (element.branchId) target.add(element.branchId);
  });
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
  const collapsedBranchIds = new Set<string>();

  // Build parent-child map for efficient lookups
  const parentChildMap = buildParentChildMap(nodes);

  // Find all collapsed operator nodes and collapsed structure nodes
  nodes.forEach((node) => {
    if (isOperatorNode(node)) {
      if (node.data.collapsed) {
        node.data.cells.forEach((cell) => {
          collectCellBranchIds(cell, collapsedBranchIds);
        });
      }
    } else if (isStructureNode(node)) {
      if (node.data.collapsed && node.data.elements) {
        collectStructureBranchIds(node.data.elements, collapsedBranchIds);
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

  // Mark collapsed branch nodes and their descendants as hidden
  collapsedBranchIds.forEach((branchId) => {
    hiddenIds.add(branchId);
    markDescendantsHidden(branchId);
  });

  return hiddenIds;
}
