// Layout configuration constants for the dagre tree layout

// Node dimension constraints
export const NODE_DIMENSIONS = {
  minWidth: 160,
  maxWidth: 450,
  defaultHeight: 80,
} as const;

// Vertical cell node specific dimensions
export const VERTICAL_CELL_DIMENSIONS = {
  rowHeight: 32,
  headerHeight: 28,
  collapsedBodyHeight: 40,
} as const;

// Text measurement estimates for width calculation
export const TEXT_METRICS = {
  // Monospace font width (~8px per character for safety margin)
  charWidthMono: 8,
  // Regular font width (~7.5px per character)
  charWidthRegular: 7.5,
  // Header label font width (16px bold font with letter-spacing)
  charWidthHeader: 10,
} as const;

// Padding and spacing for nodes
export const NODE_PADDING = {
  // Base chrome reserved around a node's widest content line: 2px border +
  // header/row left+right padding (~27px) + the header icon and its gaps. A
  // collapse toggle is NOT included here — it's added on top only for
  // collapsible (multi-cell) cards in calculateNodeWidth, so single-cell and
  // compact nodes don't carry width they never draw.
  contentPadding: 52,
  // Icon width estimate
  iconWidth: 30,
  // Type icon prefix width
  typeIconWidth: 25,
  // Collapse toggle shown in a multi-cell card header (20px button + gap).
  collapseButtonWidth: 22,
} as const;

// Dagre layout algorithm options
export const DAGRE_OPTIONS = {
  // Gap between ranks (horizontal for LR/RL layout). This IS the visible edge
  // length between a parent and its child, so keep it as tight as legibility
  // allows — every parent->child hop spans exactly this much whitespace.
  rankSep: 50,
  // Vertical separation between nodes in the same rank (also the repack gap in
  // fixChildOrdering / centerParentsOnChildren). Tighter = siblings packed
  // closer, shorter diagonal connectors.
  nodeSep: 28,
  // Separation between edges
  edgeSep: 20,
  // Margins
  marginX: 50,
  marginY: 50,
  // Layout direction: RL places the ROOT on the right and leaves/sources on the
  // left, so DATA flows left -> right (sources in, result out) per the Signal
  // Board design. Handles are mirrored to match (branch=left, input=right).
  rankDir: 'RL' as const,
} as const;

// Fixed widths for specific node types
export const FIXED_WIDTHS = {
  fallbackNode: 100,
  decisionNode: 180,
} as const;

// Decision node specific dimensions
export const DECISION_NODE_DIMENSIONS = {
  minWidth: 180,
  // Header (28px) + 3 rows (32px each) + padding (8px) = 132px
  height: 132,
} as const;

// Fixed-silhouette shapes: the boolean gate (AND/OR/NOT) and the decision
// diamond (if/elif). Their SVG carries the shape and the box is otherwise empty,
// so the layout must reserve ONLY the silhouette's true size — a box larger than
// the drawn shape just pushes neighbours away and lengthens every connector.
// The node components pin these same sizes, so reserved == drawn (no phantom gap).
export const GATE_NODE = {
  // matches .dl-gate-node min-width; the AND/OR/NOT label + silhouette fit in it
  width: 84,
} as const;

// A gate's height grows with its input count so the per-child ports down the
// left edge stay ~15px apart; kept as tight as the ports allow (NOT/2-input
// gates sit at the 44px floor).
export function gateNodeHeight(cellCount: number): number {
  return Math.max(44, cellCount * 16 + 12);
}

// The decision diamond is a fixed square drawn as a diamond. 72px leaves room
// for the "elif" label at the diamond's widest (vertical-centre) span and for
// one port per side, while sitting far tighter than the old 90px box.
export const DECISION_NODE = {
  width: 72,
  height: 72,
} as const;

// React Flow view options
export const REACT_FLOW_OPTIONS = {
  fitViewPadding: 0.2,
  maxZoom: 0.75,
} as const;
