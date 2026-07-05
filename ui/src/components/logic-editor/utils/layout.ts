import Dagre from '@dagrejs/dagre';
import type { LogicNode, LogicEdge, OperatorNodeData, StructureNodeData, CellData } from '../types';

type DagreGraph = InstanceType<typeof Dagre.graphlib.Graph>;
interface DagreNode { x: number; y: number; width: number; height: number }
import {
  NODE_DIMENSIONS,
  VERTICAL_CELL_DIMENSIONS,
  TEXT_METRICS,
  NODE_PADDING,
  DAGRE_OPTIONS,
  FIXED_WIDTHS,
  GATE_NODE,
  gateNodeHeight,
  DECISION_NODE,
} from '../constants';
import { isOperatorNode, isLiteralNode, isStructureNode } from './type-guards';
import { operatorRenderKind, cellDisplayText } from './nodeShape';
import type { FlowDirection } from '../context/DirectionContextDef';

// Compact single-row shapes render far smaller than the old header+rows card;
// the layout must reserve their TRUE size or the ReactFlow node wrapper (which is
// the drag/hit target) leaves a large dead area around the visible chip.
const COMPACT_HEIGHT = { tap: 30, infix: 38, literal: 30 } as const;

// A wired operand renders as a compact "⤷ extends to child" chip (see CellRow),
// NOT its full summary label — so the card must reserve the chip's width, not the
// (often long) hidden summary text, or it balloons far past its visible content.
const CHILD_CHIP_WIDTH = 34; // ⤷ pill: ~9px pad + 13px glyph + ~9px pad + gap

// Estimate text width based on content
function estimateTextWidth(text: string, isMonospace = false, isHeader = false): number {
  const charWidth = isMonospace
    ? TEXT_METRICS.charWidthMono
    : isHeader
      ? TEXT_METRICS.charWidthHeader
      : TEXT_METRICS.charWidthRegular;
  return text.length * charWidth;
}

// The text a cell actually renders in the card body (mirrors CellRow). Returns
// null for a wired operand, which collapses to a fixed ⤷ chip instead of text —
// so the sizer measures VISIBLE content, keeping the card width content-aware.
function cellVisibleText(cell: CellData): string | null {
  const hasAnyBranch =
    cell.type === 'branch' || !!cell.conditionBranchId || !!cell.thenBranchId;
  if (cell.type !== 'editable' && hasAnyBranch) return null; // renders as ⤷ chip
  return cell.type === 'editable'
    ? cell.label || String(cell.value ?? cell.placeholder ?? '...')
    : cell.label || cell.summary?.label || '...';
}

// Calculate width needed for a node based on its content
function calculateNodeWidth(node: LogicNode): number {
  let contentWidth: number;
  // Base chrome; a collapsible card also reserves its header collapse toggle so
  // only nodes that actually draw the toggle pay for its width.
  let chrome = NODE_PADDING.contentPadding;

  if (isOperatorNode(node)) {
    const opData = node.data as OperatorNodeData;
    // Width based on header label (header font) and cell contents
    let maxCellWidth = estimateTextWidth(opData.label, false, true);
    if (opData.cells.length > 1) chrome += NODE_PADDING.collapseButtonWidth;

    if (!opData.collapsed) {
      opData.cells.forEach((cell) => {
        // Leading row keyword ("If"/"Then"/…) sits before the pill/chip, so
        // reserve it too; wired operands render as a fixed chip, not their text.
        const rowLabelWidth = estimateTextWidth(cell.rowLabel ?? '');
        const text = cellVisibleText(cell);
        const contentPart =
          text === null ? CHILD_CHIP_WIDTH : estimateTextWidth(text, true);
        const cellWidth = NODE_PADDING.iconWidth + rowLabelWidth + contentPart;
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

  // Add chrome padding and clamp to min/max
  const totalWidth = contentWidth + chrome;
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
  // Literal — a compact folded tag: ptag + value.
  if (isLiteralNode(node)) {
    const valueStr = JSON.stringify(node.data.value);
    const w = 24 /* ptag */ + estimateTextWidth(valueStr, true) + 30 /* padding */;
    return { width: Math.max(52, Math.min(320, w)), height: COMPACT_HEIGHT.literal };
  }

  if (isOperatorNode(node)) {
    const opData = node.data as OperatorNodeData;
    const kind = operatorRenderKind(opData);

    // Data tap (var/val/exists) — a compact teal plug: ⌁ glyph + path.
    if (kind === 'tap') {
      let w = 44; // glyph + gaps + left pad + right plug point
      opData.cells.forEach((c, i) => {
        w += estimateTextWidth(cellDisplayText(c), true) + (i > 0 ? 22 : 0);
      });
      return { width: Math.max(52, w), height: COMPACT_HEIGHT.tap };
    }

    // Boolean logic-gate silhouette — reserve ONLY the drawn silhouette (the
    // component pins the same minHeight), so no empty box pads the connectors.
    // Height grows with input count to keep the left-edge ports spread out.
    if (kind === 'gate-shape') {
      return { width: GATE_NODE.width, height: gateNodeHeight(opData.cells.length) };
    }

    // Decision diamond (if / else-if) — a fixed square that draws as a diamond.
    // Matches the .dl-decision-node CSS min box so dagre reserves its true size.
    if (kind === 'decision') {
      return { width: DECISION_NODE.width, height: DECISION_NODE.height };
    }

    // Compare / arithmetic — a compact infix chip: operand OP operand.
    if (kind === 'infix-gate' || kind === 'infix-arith') {
      const isGate = kind === 'infix-gate';
      let w = 30; // chip padding + signal rail
      if (isGate) w += 16; // LED
      const unary = opData.cells.length === 1;
      opData.cells.forEach((cell, i) => {
        if (unary || i > 0) w += isGate ? 24 : 32; // glyph / round op badge (+ gaps)
        w += estimateTextWidth(cellDisplayText(cell), true) + 20; // pill + padding + gap
      });
      return {
        width: Math.max(64, Math.min(NODE_DIMENSIONS.maxWidth, w)),
        height: COMPACT_HEIGHT.infix,
      };
    }

    // Card (if / and / or / value / iterator) — header + stacked rows.
    const width = calculateNodeWidth(node);
    if (opData.cells.length > 0) {
      if (opData.collapsed) {
        return {
          width,
          height: VERTICAL_CELL_DIMENSIONS.headerHeight + VERTICAL_CELL_DIMENSIONS.collapsedBodyHeight,
        };
      }
      // Header ~32 + body padding 8 + per-row reserve. Most rows are a ~30px
      // pill/chip; a condition row hosts a 40px decision diamond (switch/match
      // cards). Reserving each row's TRUE height keeps siblings from drifting
      // apart (over-reserve) or overlapping (under-reserve).
      const rowsHeight = opData.cells.reduce(
        (sum, cell) => sum + (cell.icon === 'diamond' ? 46 : 30),
        0,
      );
      return {
        width,
        height: 32 + 8 + rowsHeight,
      };
    }
    return { width, height: NODE_DIMENSIONS.defaultHeight };
  }

  const width = calculateNodeWidth(node);
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
export function applyTreeLayout(
  nodes: LogicNode[],
  edges?: LogicEdge[],
  direction: FlowDirection = 'flow',
): LogicNode[] {
  if (nodes.length === 0) return nodes;

  // Create a new dagre graph
  const g = new Dagre.graphlib.Graph().setDefaultEdgeLabel(() => ({}));

  // rankDir alone flips root side: with parent->child edges, 'RL' ranks the root
  // on the right (data flow), 'LR' ranks it on the left (JSON hierarchy).
  const rankdir = direction === 'hierarchy' ? 'LR' : DAGRE_OPTIONS.rankDir;

  // Set graph options
  g.setGraph({
    rankdir,
    nodesep: DAGRE_OPTIONS.nodeSep,
    ranksep: DAGRE_OPTIONS.rankSep,
    edgesep: DAGRE_OPTIONS.edgeSep,
    marginx: DAGRE_OPTIONS.marginX,
    marginy: DAGRE_OPTIONS.marginY,
    // A logic graph is a pure tree, so a tight spanning tree ranks it as
    // compactly as network-simplex while packing ranks a touch closer.
    ranker: 'tight-tree',
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

  // Parent -> ordered children, shared by both post-passes below.
  const childrenMap = buildChildrenMap(nodes, nodeIdSet);

  // Post-process 1: reorder children to match handle (port) order.
  // Dagre treats nodes as single vertices and doesn't know about handle positions,
  // so children may be placed in a vertical order that doesn't match their
  // source handle positions on the parent, causing edges to cross.
  //
  // This also gives an if / else-if chain its shape for free: the else child (the
  // next diamond) is ordered last, so each diamond lands one rank left and below
  // its parent — a compact descending cascade (when above, then level-left, else
  // into the diamond below), rather than a straight rightward row.
  fixChildOrdering(g, childrenMap);

  // Post-process 2: re-center each parent on its children (leaves -> root) to
  // straighten and shorten the connectors. Dagre (and the reorder above) leave a
  // parent off its children's midpoint; pulling it back onto centre removes that
  // slack — the single biggest win for "minimum connection length".
  centerParentsOnChildren(g, nodes, childrenMap);

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
  g: DagreGraph,
  childrenMap: Map<string, string[]>
): { minY: number; maxY: number } {
  const node = g.node(nodeId) as DagreNode;
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
  g: DagreGraph,
  childrenMap: Map<string, string[]>
): void {
  (g.node(nodeId) as DagreNode).y += delta;
  const descendants = new Set<string>();
  collectDescendants(nodeId, childrenMap, descendants);
  for (const descId of descendants) {
    const descNode = g.node(descId) as DagreNode | undefined;
    if (descNode) {
      descNode.y += delta;
    }
  }
}

// Build the parent -> ordered-children map used by the post-layout passes.
// Children are in visual (handle) order; only nodes that actually have children
// become keys.
function buildChildrenMap(
  nodes: LogicNode[],
  nodeIdSet: Set<string>
): Map<string, string[]> {
  const childrenMap = new Map<string, string[]>();
  for (const node of nodes) {
    const children = getChildrenInVisualOrder(node, nodeIdSet);
    if (children.length > 0) childrenMap.set(node.id, children);
  }
  return childrenMap;
}

// Reorder children to match handle order on parent nodes.
// Uses subtree-aware repacking to avoid overlaps when subtrees have different heights.
function fixChildOrdering(g: DagreGraph, childrenMap: Map<string, string[]>): void {
  const gap = DAGRE_OPTIONS.nodeSep;

  for (const orderedChildren of childrenMap.values()) {
    if (orderedChildren.length < 2) continue;

    // Check if children are already in correct Y order
    const currentYs = orderedChildren.map((id) => (g.node(id) as DagreNode).y);
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

// Re-center every parent on its immediate children, processed leaves -> root so
// the centering propagates up the tree. Only Y is touched (X = the dagre rank);
// this shortens the vertical offset of every connector without changing rank
// spacing. Within each rank the existing top-to-bottom order is preserved and a
// minimum center-to-center gap (half-heights + nodeSep) is enforced by isotonic
// (pool-adjacent-violators) placement, so pulling parents onto centre can never
// introduce an overlap or reorder a parent's children.
function centerParentsOnChildren(
  g: DagreGraph,
  nodes: LogicNode[],
  childrenMap: Map<string, string[]>
): void {
  // Depth of each node. A logic graph is a tree (one parent per node), so depth
  // == the dagre rank; roots (no parent) are depth 0.
  const parentOf = new Map<string, string>();
  for (const [parent, kids] of childrenMap) {
    for (const kid of kids) parentOf.set(kid, parent);
  }
  const depthOf = new Map<string, number>();
  const inProgress = new Set<string>();
  const depthOfId = (id: string): number => {
    const cached = depthOf.get(id);
    if (cached !== undefined) return cached;
    const parent = parentOf.get(id);
    let d = 0;
    // inProgress guards against an accidental cycle (a tree should never have one).
    if (parent !== undefined && !inProgress.has(id)) {
      inProgress.add(id);
      d = depthOfId(parent) + 1;
      inProgress.delete(id);
    }
    depthOf.set(id, d);
    return d;
  };

  // Group node ids by depth (== rank).
  const ranks = new Map<number, string[]>();
  let maxDepth = 0;
  for (const node of nodes) {
    const d = depthOfId(node.id);
    if (d > maxDepth) maxDepth = d;
    let arr = ranks.get(d);
    if (!arr) {
      arr = [];
      ranks.set(d, arr);
    }
    arr.push(node.id);
  }

  for (let d = maxDepth; d >= 0; d--) {
    const ids = ranks.get(d);
    if (!ids || ids.length === 0) continue;

    // Preserve the current top-to-bottom order (set by dagre + fixChildOrdering).
    ids.sort((a, b) => (g.node(a) as DagreNode).y - (g.node(b) as DagreNode).y);

    // Desired centre: a parent wants the midpoint of its immediate children's
    // span (children already placed, one rank deeper); a leaf keeps its own Y.
    const desired = ids.map((id) => {
      const kids = childrenMap.get(id);
      if (!kids || kids.length === 0) return (g.node(id) as DagreNode).y;
      let lo = Infinity;
      let hi = -Infinity;
      for (const kid of kids) {
        const y = (g.node(kid) as DagreNode).y;
        if (y < lo) lo = y;
        if (y > hi) hi = y;
      }
      return (lo + hi) / 2;
    });

    // Cumulative minimum center offsets: G[i] is the smallest legal center of
    // node i relative to node 0. Isotonic-regress (desired - G) into a
    // non-decreasing sequence (pool adjacent violators), then add G back — the
    // L2-closest placement to every node's desired centre that still respects
    // order and the min gap.
    const heights = ids.map((id) => (g.node(id) as DagreNode).height);
    const gapOffset: number[] = new Array(ids.length);
    gapOffset[0] = 0;
    for (let i = 1; i < ids.length; i++) {
      gapOffset[i] =
        gapOffset[i - 1] + heights[i - 1] / 2 + DAGRE_OPTIONS.nodeSep + heights[i] / 2;
    }
    const blocks: { sum: number; cnt: number; avg: number }[] = [];
    for (let i = 0; i < ids.length; i++) {
      let sum = desired[i] - gapOffset[i];
      let cnt = 1;
      while (blocks.length > 0 && blocks[blocks.length - 1].avg > sum / cnt) {
        const prev = blocks.pop()!;
        sum += prev.sum;
        cnt += prev.cnt;
      }
      blocks.push({ sum, cnt, avg: sum / cnt });
    }
    let i = 0;
    for (const block of blocks) {
      for (let k = 0; k < block.cnt; k++) {
        (g.node(ids[i]) as DagreNode).y = block.avg + gapOffset[i];
        i++;
      }
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
