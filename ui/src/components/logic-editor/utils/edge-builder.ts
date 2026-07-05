import type { LogicNode, LogicEdge, OperatorNodeData, StructureNodeData } from '../types';
import type { FlowDirection } from '../context/DirectionContextDef';

/**
 * An edge between a parent operator and one of its operand children, oriented for
 * the current direction. The child always connects at its 'left' handle (its
 * result), the parent at its per-cell 'branch-*' handle:
 *  - flow      — child → parent, so the arrowhead lands on the parent (points
 *                right, toward the result on the right).
 *  - hierarchy — parent → child, so the arrowhead lands on the child (points
 *                right, toward the leaves; root reads on the left).
 */
function orientedEdge(
  id: string,
  parentId: string,
  childId: string,
  parentHandle: string,
  direction: FlowDirection,
): LogicEdge {
  return direction === 'hierarchy'
    ? { id, source: parentId, target: childId, sourceHandle: parentHandle, targetHandle: 'left' }
    : { id, source: childId, target: parentId, sourceHandle: 'left', targetHandle: parentHandle };
}

/**
 * Build edges from node relationships, respecting collapse state. This creates
 * edges based on the current node data, including collapsed cells and nodes.
 * The orientation (which end carries the arrowhead) follows `direction`.
 */
export function buildEdgesFromNodes(
  nodes: LogicNode[],
  direction: FlowDirection = 'flow',
): LogicEdge[] {
  const edges: LogicEdge[] = [];

  nodes.forEach((node) => {
    // Handle operator nodes with cells
    if (node.type === 'operator') {
      const opData = node.data as OperatorNodeData;
      if (!opData.collapsed) {
        opData.cells.forEach((cell) => {
          // 1. Condition branch (if exists)
          if (cell.conditionBranchId) {
            edges.push(
              orientedEdge(
                `${node.id}-cond-${cell.conditionBranchId}`,
                node.id,
                cell.conditionBranchId,
                `branch-${cell.index}-cond`,
                direction,
              ),
            );
          }
          // 2. Then branch (if exists)
          if (cell.thenBranchId) {
            edges.push(
              orientedEdge(
                `${node.id}-then-${cell.thenBranchId}`,
                node.id,
                cell.thenBranchId,
                `branch-${cell.index}-then`,
                direction,
              ),
            );
          }
          // 3. Standard branch - ONLY if no condition/then (mutually exclusive)
          if (cell.branchId && !cell.conditionBranchId && !cell.thenBranchId) {
            edges.push(
              orientedEdge(
                `${node.id}-branch-${cell.branchId}`,
                node.id,
                cell.branchId,
                `branch-${cell.index}`,
                direction,
              ),
            );
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
            edges.push(
              orientedEdge(
                `${node.id}-expr-${element.branchId}`,
                node.id,
                element.branchId,
                `branch-${idx}`,
                direction,
              ),
            );
          }
        });
      }
    }
  });

  return edges;
}
