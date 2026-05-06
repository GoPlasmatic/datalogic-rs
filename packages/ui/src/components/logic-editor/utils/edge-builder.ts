import type { LogicNode, LogicEdge, OperatorNodeData, StructureNodeData } from '../types';

/**
 * Build edges from node relationships, respecting collapse state.
 * This function creates edges based on the current node data, including
 * collapsed cells and collapsed nodes.
 */
export function buildEdgesFromNodes(nodes: LogicNode[]): LogicEdge[] {
  const edges: LogicEdge[] = [];

  nodes.forEach((node) => {
    // Handle operator nodes with cells
    if (node.type === 'operator') {
      const opData = node.data as OperatorNodeData;
      if (!opData.collapsed) {
        opData.cells.forEach((cell) => {
          // Use cell.index for stable handle IDs
          // Handle IDs match CellHandles.tsx: branch-{cellIndex}, branch-{cellIndex}-cond, branch-{cellIndex}-then

          // 1. Condition branch (if exists)
          if (cell.conditionBranchId) {
            edges.push({
              id: `${node.id}-cond-${cell.conditionBranchId}`,
              source: node.id,
              target: cell.conditionBranchId,
              sourceHandle: `branch-${cell.index}-cond`,
              targetHandle: 'left',
            });
          }
          // 2. Then branch (if exists)
          if (cell.thenBranchId) {
            edges.push({
              id: `${node.id}-then-${cell.thenBranchId}`,
              source: node.id,
              target: cell.thenBranchId,
              sourceHandle: `branch-${cell.index}-then`,
              targetHandle: 'left',
            });
          }
          // 3. Standard branch - ONLY if no condition/then (mutually exclusive)
          if (cell.branchId && !cell.conditionBranchId && !cell.thenBranchId) {
            edges.push({
              id: `${node.id}-branch-${cell.branchId}`,
              source: node.id,
              target: cell.branchId,
              sourceHandle: `branch-${cell.index}`,
              targetHandle: 'left',
            });
          }
        });
      }
    }

    // Handle structure nodes with expression branches
    if (node.type === 'structure') {
      const structData = node.data as StructureNodeData;
      if (!structData.collapsed && structData.elements) {
        // Filter to only expression elements that have branchIds
        const expressionElements = structData.elements.filter(
          (el) => el.type === 'expression' && el.branchId
        );
        expressionElements.forEach((element, idx) => {
          if (element.branchId) {
            edges.push({
              id: `${node.id}-expr-${element.branchId}`,
              source: node.id,
              target: element.branchId,
              sourceHandle: `branch-${idx}`,
              targetHandle: 'left',
            });
          }
        });
      }
    }
  });

  return edges;
}
