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
  // Total padding for borders, icons, margins, collapse buttons, result badges
  contentPadding: 80,
  // Icon width estimate
  iconWidth: 30,
  // Type icon prefix width
  typeIconWidth: 25,
} as const;

// Dagre layout algorithm options
export const DAGRE_OPTIONS = {
  // Gap between ranks (horizontal for LR layout)
  rankSep: 80,
  // Vertical separation between nodes in same rank
  nodeSep: 40,
  // Separation between edges
  edgeSep: 20,
  // Margins
  marginX: 50,
  marginY: 50,
  // Layout direction (left-to-right for horizontal flow)
  rankDir: 'LR' as const,
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

// React Flow view options
export const REACT_FLOW_OPTIONS = {
  fitViewPadding: 0.2,
  maxZoom: 0.75,
} as const;
