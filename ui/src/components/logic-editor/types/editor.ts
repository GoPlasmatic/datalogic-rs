import type { Node, Edge } from '@xyflow/react';
import type { OperatorCategory, JsonLogicValue } from './jsonlogic';
import type { IconName } from '../utils/icons';
import type { TracedResult } from './trace';

// Visual node types
export type VisualNodeType = 'operator' | 'variable' | 'literal' | 'verticalCell' | 'decision' | 'structure';

// Base data for all visual nodes
// Note: Index signature is required for React Flow Node type compatibility
export interface BaseNodeData extends Record<string, unknown> {
  type: VisualNodeType;
  parentId?: string;
  argIndex?: number;
  branchType?: 'yes' | 'no'; // For decision tree branches
  expression?: JsonLogicValue; // Original JSONLogic expression for this node (for debugging)
}

// Argument summary for collapsed view
export interface ArgSummary {
  icon: IconName; // Type icon (Lucide icon name)
  label: string; // Human-readable summary text
  valueType: 'string' | 'number' | 'boolean' | 'null' | 'array' | 'date' | 'expression';
}

// Operator node data
export interface OperatorNodeData extends BaseNodeData {
  type: 'operator';
  operator: string;
  category: OperatorCategory;
  label: string;
  childIds: string[];
  collapsed?: boolean; // Whether the node is collapsed
  expressionText?: string; // Full expression as single-line text when collapsed
  inlineDisplay?: string; // For unary operators with simple args - shows inline without expansion
}

// Variable node data (var, val, exists)
export interface VariableNodeData extends BaseNodeData {
  type: 'variable';
  operator: 'var' | 'val' | 'exists';
  path: string;
  defaultValue?: JsonLogicValue;
}

// Literal value node data
export interface LiteralNodeData extends BaseNodeData {
  type: 'literal';
  value: JsonLogicValue;
  valueType: 'string' | 'number' | 'boolean' | 'null' | 'array';
}

// Cell data for vertical cell nodes
export interface CellData {
  type: 'inline' | 'branch';
  rowLabel?: string; // Row keyword label ("If", "Then", "Else If", "Else")
  label?: string; // Display text for inline cells (expression text)
  icon?: IconName; // Optional Lucide icon name
  branchId?: string; // For branch cells, the ID of the sub-expression node
  index: number; // Original argument index
  summary?: ArgSummary; // Summary for branch cells to show when collapsed
  // For if/then cells - support condition and then branches separately
  conditionBranchId?: string; // Branch for condition expression
  thenBranchId?: string; // Branch for then value (Yes)
  conditionText?: string; // Condition expression text
  thenText?: string; // Then value text
}

// Vertical cell node data (for comparison chains, logical operators, iterators)
export interface VerticalCellNodeData extends BaseNodeData {
  type: 'verticalCell';
  operator: string;
  category: OperatorCategory;
  label: string;
  icon: IconName; // Category Lucide icon name
  cells: CellData[];
  collapsed?: boolean; // Whether the entire node is collapsed
  expressionText?: string; // Full expression as single-line text when collapsed
  collapsedCellIndices?: number[]; // Indices of collapsed branch cells (when expanded)
}

// Decision node data (for if/then/else decision tree visualization)
export interface DecisionNodeData extends BaseNodeData {
  type: 'decision';
  conditionText: string; // Display text for condition
  conditionExpression: JsonLogicValue; // Original condition for complex branching
  isConditionComplex: boolean; // If true, condition branches to sub-graph
  conditionBranchId?: string; // ID of condition sub-graph node
  yesBranchId: string; // ID of "Yes" result node
  noBranchId: string; // ID of "No" result node (else or next decision)
  collapsed?: boolean;
  expressionText?: string; // Full if/then/else text for collapsed view
}

// Structure element - either inline value or linked expression
export interface StructureElement {
  type: 'inline' | 'expression';
  path: string[];              // JSON path to this element (e.g., ["party_identifier"])
  key?: string;                // Key name for object properties
  value?: JsonLogicValue;      // For inline values
  branchId?: string;           // For linked expressions - ID of the child node
  startOffset: number;         // Character offset in formatted JSON (for highlighting)
  endOffset: number;           // End offset for highlighting
}

// Structure node data - displays formatted JSON with linked expressions
export interface StructureNodeData extends BaseNodeData {
  type: 'structure';
  isArray: boolean;            // true for arrays, false for objects
  formattedJson: string;       // Pretty-printed JSON string with placeholders
  elements: StructureElement[]; // All elements (inline + expressions)
  collapsed?: boolean;
  expressionText?: string;     // Full expression as single-line text when collapsed
}

// Union type for all node data
export type LogicNodeData = OperatorNodeData | VariableNodeData | LiteralNodeData | VerticalCellNodeData | DecisionNodeData | StructureNodeData;

// ReactFlow node with our custom data
export type LogicNode = Node<LogicNodeData>;

// ReactFlow edge
export type LogicEdge = Edge;

// Editor state
export interface EditorState {
  nodes: LogicNode[];
  edges: LogicEdge[];
  selectedNodeId: string | null;
  editingNodeId: string | null;
}

// Conversion result from JSONLogic to visual nodes
export interface ConversionResult {
  nodes: LogicNode[];
  edges: LogicEdge[];
  rootId: string | null;
}

// Evaluation result for debugging
export interface NodeEvaluationResult {
  value: unknown;
  error: string | null;
  type: 'boolean' | 'number' | 'string' | 'null' | 'array' | 'object' | 'undefined';
}

// Map of node ID to evaluation result
export type EvaluationResultsMap = Map<string, NodeEvaluationResult>;

// Props for the LogicEditor component (internal use, legacy)
export interface LogicEditorProps {
  value: JsonLogicValue | null;
  onChange: (expr: JsonLogicValue | null) => void;
  readOnly?: boolean;
  className?: string;
  evaluationResults?: EvaluationResultsMap;
  /** Data object for debug evaluation */
  debugData?: unknown;
  /** Evaluate function from WASM - if provided, enables debug mode */
  evaluate?: (logic: unknown, data: unknown) => unknown;
  /** Evaluate with trace function from WASM - if provided, uses trace API for diagram rendering */
  evaluateWithTrace?: (logic: unknown, data: unknown) => TracedResult;
}

/**
 * Editor operating mode:
 * - 'visualize' (ReadOnly): Static diagram visualization, no evaluation
 * - 'debug' (Debugger): Diagram with evaluation results and step-through debugging
 * - 'edit' (Editor+Debugger): Coming Soon - Full visual builder with live evaluation
 */
export type DataLogicEditorMode = 'visualize' | 'debug' | 'edit';

/**
 * Component configuration mode:
 * - 'debugger': Shows mode selector, allows switching between view/debug
 * - 'visualizer': Hides mode selector, fixed to view mode only
 */
export type DataLogicComponentMode = 'debugger' | 'visualizer';

/**
 * Props for the DataLogicEditor component (public API)
 */
export interface DataLogicEditorProps {
  /** JSONLogic expression to render */
  value: JsonLogicValue | null;

  /** Callback when expression changes (only in 'edit' mode - Coming Soon) */
  onChange?: (expr: JsonLogicValue | null) => void;

  /** Data context for evaluation (used in 'debug' and 'edit' modes) */
  data?: unknown;

  /**
   * Editor operating mode:
   * - 'visualize' (default): Static diagram visualization, no evaluation
   * - 'debug': Diagram with evaluation results and step-through debugging
   * - 'edit': Coming Soon - Full visual builder with live evaluation
   */
  mode?: DataLogicEditorMode;

  /** Theme override - 'light' or 'dark'. If not provided, uses system preference */
  theme?: 'light' | 'dark';

  /** Additional CSS class */
  className?: string;

  /**
   * Enable structure preserve mode for JSON templates with embedded JSONLogic.
   * When true, multi-key objects and arrays are treated as data structures
   * with embedded JSONLogic expressions, rather than invalid JSONLogic.
   */
  preserveStructure?: boolean;

  /**
   * Component mode configuration:
   * - 'debugger' (default): Shows mode selector for view/debug switching
   * - 'visualizer': Pure visualization mode, no mode selector shown
   */
  componentMode?: DataLogicComponentMode;
}
