import Dagre from '@dagrejs/dagre';
import type { LogicNode, LogicEdge, OperatorNodeData, StructureNodeData } from '../types';
import {
  NODE_DIMENSIONS,
  VERTICAL_CELL_DIMENSIONS,
  TEXT_METRICS,
  NODE_PADDING,
  DAGRE_OPTIONS,
  FIXED_WIDTHS,
} from '../constants';
import { isOperatorNode, isLiteralNode, isStructureNode } from './type-guards';
import { getOperator } from '../config/operators';

// Estimate text width based on content
function estimateTextWidth(text: string, isMonospace = false, isHeader = false): number {
  const charWidth = isMonospace
    ? TEXT_METRICS.charWidthMono
    : isHeader
      ? TEXT_METRICS.charWidthHeader
      : TEXT_METRICS.charWidthRegular;
  return text.length * charWidth;
}

// Calculate width needed for a node based on its content
function calculateNodeWidth(node: LogicNode): number {
  let contentWidth: number;

  if (isOperatorNode(node)) {
    const opData = node.data as OperatorNodeData;
    // Width based on header label (header font) and cell contents
    let maxCellWidth = estimateTextWidth(opData.label, false, true);

    if (!opData.collapsed) {
      opData.cells.forEach((cell) => {
        const cellText = cell.label || cell.summary?.label || '';
        const cellWidth = NODE_PADDING.iconWidth + estimateTextWidth(cellText, true);
        maxCellWidth = Math.max(maxCellWidth, cellWidth);
      });
    } else if (opData.expressionText) {
      maxCellWidth = Math.max(maxCellWidth, estimateTextWidth(opData.expressionText, true));
    }
    contentWidth = maxCellWidth;
  } else if (isLiteralNode(node)) {
    const litData = node.data;
    // Width based on value display
    const valueStr = JSON.stringify(litData.value);
    contentWidth = NODE_PADDING.typeIconWidth + estimateTextWidth(valueStr, true);
  } else if (isStructureNode(node)) {
    const structData = node.data;
    if (structData.collapsed && structData.expressionText) {
      // Collapsed: use expression text width
      contentWidth = estimateTextWidth(structData.expressionText, true);
    } else {
      // Expanded: calculate width based on formatted JSON lines
      const lines = structData.formattedJson.split('\n');
      let maxLineWidth = 0;
      for (const line of lines) {
        const lineWidth = estimateTextWidth(line, true);
        maxLineWidth = Math.max(maxLineWidth, lineWidth);
      }
      contentWidth = maxLineWidth;
    }
  } else {
    contentWidth = FIXED_WIDTHS.fallbackNode;
  }

  // Add padding and clamp to min/max
  const totalWidth = contentWidth + NODE_PADDING.contentPadding;
  return Math.max(NODE_DIMENSIONS.minWidth, Math.min(NODE_DIMENSIONS.maxWidth, totalWidth));
}

// Structure node dimensions
const STRUCTURE_DIMENSIONS = {
  headerHeight: 32,
  lineHeight: 18,
  bodyPadding: 16, // 8px top + 8px bottom
  collapsedBodyHeight: 30,
};

// Get node dimensions based on type and content
function getNodeDimensions(node: LogicNode): { width: number; height: number } {
  const width = calculateNodeWidth(node);

  if (isOperatorNode(node)) {
    const opData = node.data as OperatorNodeData;
    if (opData.cells.length > 0) {
      if (opData.collapsed) {
        return {
          width,
          height: VERTICAL_CELL_DIMENSIONS.headerHeight + VERTICAL_CELL_DIMENSIONS.collapsedBodyHeight,
        };
      }
      const cellCount = opData.cells.length;
      // Body padding: 4px top + 4px bottom = 8px
      const bodyPadding = 8;
      // Add button height: padding(4+4) + font(~14) + margin(4+8) = ~34px
      // Show for variable-arity operators (nary, variadic, chainable, special, range)
      const opConfig = getOperator(opData.operator);
      const arityType = opConfig?.arity?.type;
      const hasAddButton = arityType === 'nary' || arityType === 'variadic' ||
        arityType === 'chainable' || arityType === 'special' || arityType === 'range';
      const addButtonHeight = hasAddButton ? 34 : 0;
      return {
        width,
        height: VERTICAL_CELL_DIMENSIONS.headerHeight + bodyPadding + cellCount * VERTICAL_CELL_DIMENSIONS.rowHeight + addButtonHeight,
      };
    }
    return { width, height: NODE_DIMENSIONS.defaultHeight };
  }

  if (isStructureNode(node)) {
    const structData = node.data;
    if (structData.collapsed) {
      return {
        width,
        height: STRUCTURE_DIMENSIONS.headerHeight + STRUCTURE_DIMENSIONS.collapsedBodyHeight,
      };
    }
    const lineCount = structData.formattedJson.split('\n').length;
    return {
      width,
      height: STRUCTURE_DIMENSIONS.headerHeight + STRUCTURE_DIMENSIONS.bodyPadding + lineCount * STRUCTURE_DIMENSIONS.lineHeight,
    };
  }

  return { width, height: NODE_DIMENSIONS.defaultHeight };
}

// Apply dagre layout to nodes and edges
export function applyTreeLayout(nodes: LogicNode[], edges?: LogicEdge[]): LogicNode[] {
  if (nodes.length === 0) return nodes;

  // Create a new dagre graph
  const g = new Dagre.graphlib.Graph().setDefaultEdgeLabel(() => ({}));

  // Set graph options - use LR (left-to-right) for horizontal flow
  g.setGraph({
    rankdir: DAGRE_OPTIONS.rankDir,
    nodesep: DAGRE_OPTIONS.nodeSep,
    ranksep: DAGRE_OPTIONS.rankSep,
    edgesep: DAGRE_OPTIONS.edgeSep,
    marginx: DAGRE_OPTIONS.marginX,
    marginy: DAGRE_OPTIONS.marginY,
  });

  // Add nodes to the graph with their dimensions
  nodes.forEach((node) => {
    const { width, height } = getNodeDimensions(node);
    g.setNode(node.id, { width, height });
  });

  // Build edges from node relationships if edges not provided
  const edgesToUse = edges || buildEdgesFromNodes(nodes);

  // Build set of valid node IDs for edge validation
  const nodeIdSet = new Set(nodes.map((n) => n.id));

  // Add edges to the graph (only if both source and target exist)
  // Track edge index per source node to help Dagre maintain child order
  const sourceEdgeCount = new Map<string, number>();
  edgesToUse.forEach((edge) => {
    if (nodeIdSet.has(edge.source) && nodeIdSet.has(edge.target)) {
      // Get current edge index for this source node
      const edgeIndex = sourceEdgeCount.get(edge.source) || 0;
      sourceEdgeCount.set(edge.source, edgeIndex + 1);

      // Pass edge weight to help maintain order (higher index = higher weight = lower position)
      g.setEdge(edge.source, edge.target, { weight: edgeIndex + 1 });
    }
  });

  // Run the dagre layout algorithm
  Dagre.layout(g);

  // Post-process: reorder children to match handle (port) order.
  // Dagre treats nodes as single vertices and doesn't know about handle positions,
  // so children may be placed in a vertical order that doesn't match their
  // source handle positions on the parent, causing edges to cross.
  fixChildOrdering(g, nodes, nodeIdSet);

  // Apply the calculated positions and dimensions to nodes
  return nodes.map((node) => {
    const nodeWithPosition = g.node(node.id);
    if (nodeWithPosition) {
      const { width, height } = getNodeDimensions(node);
      return {
        ...node,
        position: {
          // Dagre returns center positions, convert to top-left
          x: nodeWithPosition.x - width / 2,
          y: nodeWithPosition.y - height / 2,
        },
        // Store calculated dimensions for node components to use
        width,
        height,
        style: {
          width: `${width}px`,
        },
      };
    }
    return node;
  });
}

// Get children of a node in their visual (top-to-bottom) order based on cell/element data
function getChildrenInVisualOrder(node: LogicNode, nodeIdSet: Set<string>): string[] {
  if (isOperatorNode(node)) {
    const opData = node.data as OperatorNodeData;
    if (opData.collapsed) return [];
    const children: string[] = [];
    for (const cell of opData.cells) {
      // Condition handle is above then handle within the same row
      if (cell.conditionBranchId && nodeIdSet.has(cell.conditionBranchId)) {
        children.push(cell.conditionBranchId);
      }
      if (cell.thenBranchId && nodeIdSet.has(cell.thenBranchId)) {
        children.push(cell.thenBranchId);
      }
      // Standard branch (mutually exclusive with condition/then)
      if (cell.branchId && !cell.conditionBranchId && !cell.thenBranchId && nodeIdSet.has(cell.branchId)) {
        children.push(cell.branchId);
      }
    }
    return children;
  }

  if (isStructureNode(node)) {
    const structData = node.data as StructureNodeData;
    if (structData.collapsed || !structData.elements) return [];
    return structData.elements
      .filter((el) => el.type === 'expression' && el.branchId && nodeIdSet.has(el.branchId))
      .map((el) => el.branchId!);
  }

  return [];
}

// Recursively collect all descendant node IDs
function collectDescendants(
  nodeId: string,
  childrenMap: Map<string, string[]>,
  result: Set<string>
): void {
  const children = childrenMap.get(nodeId);
  if (!children) return;
  for (const child of children) {
    if (!result.has(child)) {
      result.add(child);
      collectDescendants(child, childrenMap, result);
    }
  }
}

// Compute the vertical extent (bounding box) of a node and all its descendants
function computeSubtreeExtent(
  nodeId: string,
  g: Dagre.graphlib.Graph,
  childrenMap: Map<string, string[]>
): { minY: number; maxY: number } {
  const node = g.node(nodeId);
  const halfHeight = (node.height || 0) / 2;
  let minY = node.y - halfHeight;
  let maxY = node.y + halfHeight;

  const children = childrenMap.get(nodeId);
  if (children) {
    for (const childId of children) {
      const childExtent = computeSubtreeExtent(childId, g, childrenMap);
      minY = Math.min(minY, childExtent.minY);
      maxY = Math.max(maxY, childExtent.maxY);
    }
  }

  return { minY, maxY };
}

// Shift a node and all its descendants by a Y delta
function shiftSubtree(
  nodeId: string,
  delta: number,
  g: Dagre.graphlib.Graph,
  childrenMap: Map<string, string[]>
): void {
  g.node(nodeId).y += delta;
  const descendants = new Set<string>();
  collectDescendants(nodeId, childrenMap, descendants);
  for (const descId of descendants) {
    const descNode = g.node(descId);
    if (descNode) {
      descNode.y += delta;
    }
  }
}

// Reorder children to match handle order on parent nodes.
// Uses subtree-aware repacking to avoid overlaps when subtrees have different heights.
function fixChildOrdering(
  g: Dagre.graphlib.Graph,
  nodes: LogicNode[],
  nodeIdSet: Set<string>
): void {
  // Build children map and collect parents that need ordering fixes
  const childrenMap = new Map<string, string[]>();
  const parentsToFix: { orderedChildren: string[] }[] = [];

  for (const node of nodes) {
    const children = getChildrenInVisualOrder(node, nodeIdSet);
    if (children.length > 0) {
      childrenMap.set(node.id, children);
      if (children.length >= 2) {
        parentsToFix.push({ orderedChildren: children });
      }
    }
  }

  const gap = DAGRE_OPTIONS.nodeSep;

  for (const { orderedChildren } of parentsToFix) {
    // Check if children are already in correct Y order
    const currentYs = orderedChildren.map((id) => g.node(id).y);
    let needsFix = false;
    for (let i = 1; i < currentYs.length; i++) {
      if (currentYs[i] < currentYs[i - 1]) {
        needsFix = true;
        break;
      }
    }
    if (!needsFix) continue;

    // Compute subtree extents for each child (in handle order)
    const subtrees = orderedChildren.map((childId) => ({
      childId,
      extent: computeSubtreeExtent(childId, g, childrenMap),
    }));

    // Find the starting Y: top of the topmost subtree in the current layout
    const overallMinY = Math.min(...subtrees.map((s) => s.extent.minY));

    // Repack subtrees top-to-bottom in handle order with proper gaps
    let currentTop = overallMinY;
    for (const subtree of subtrees) {
      const delta = currentTop - subtree.extent.minY;
      if (delta !== 0) {
        shiftSubtree(subtree.childId, delta, g, childrenMap);
      }
      const subtreeHeight = subtree.extent.maxY - subtree.extent.minY;
      currentTop += subtreeHeight + gap;
    }
  }
}

// Build edges from node parent relationships
function buildEdgesFromNodes(nodes: LogicNode[]): LogicEdge[] {
  const edges: LogicEdge[] = [];

  nodes.forEach((node) => {
    // Handle operator nodes with cells
    if (isOperatorNode(node)) {
      const opData = node.data as OperatorNodeData;
      if (!opData.collapsed) {
        opData.cells.forEach((cell) => {
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
  });

  return edges;
}
