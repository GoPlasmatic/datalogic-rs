import type { Node, Edge } from '@xyflow/react';
import type { OperatorCategory, JsonLogicValue } from './jsonlogic';
import type { IconName } from '../utils/icons';
import type { TracedResult } from './trace';

// Visual node types
export type VisualNodeType = 'operator' | 'literal' | 'structure';

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

// Unified operator node data - handles ALL operators including var, val, exists, if, etc.
export interface OperatorNodeData extends BaseNodeData {
  type: 'operator';
  operator: string;
  category: OperatorCategory;
  label: string;
  icon: IconName; // Category icon
  cells: CellData[]; // ALL arguments as rows
  collapsed?: boolean;
  expressionText?: string; // Full expression as single-line text when collapsed
}

// Literal value node data
export interface LiteralNodeData extends BaseNodeData {
  type: 'literal';
  value: JsonLogicValue;
  valueType: 'string' | 'number' | 'boolean' | 'null' | 'array';
}

// Cell data for operator node rows
export interface CellData {
  type: 'inline' | 'branch' | 'editable'; // 'editable' for var path, val scope, etc.
  rowLabel?: string; // Row keyword label ("If", "Then", "Else If", "Else", "Path", "Default")
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
  // For editable cells (var path, val scope, etc.)
  fieldId?: string; // e.g., 'path', 'default', 'scopeLevel'
  fieldType?: 'text' | 'number' | 'select'; // Input type for editable cells
  value?: unknown; // Current value for editable fields
  placeholder?: string; // Placeholder text for editable fields
}

// Variable node data extends operator node data with variable-specific fields
export interface VariableNodeData extends OperatorNodeData {
  // Variable-specific fields are now in cells
  path?: string;
  defaultValue?: JsonLogicValue;
  scopeJump?: number;
  pathComponents?: string[];
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
export type LogicNodeData = OperatorNodeData | LiteralNodeData | StructureNodeData;

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
 * Props for the DataLogicEditor component (public API)
 */
export interface DataLogicEditorProps {
  /** JSONLogic expression to render */
  value: JsonLogicValue | null;

  /** Callback when expression changes (only when editable is true) */
  onChange?: (expr: JsonLogicValue | null) => void;

  /** Data context for evaluation. When provided, debugger controls become available. */
  data?: unknown;

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

  /** Callback when preserve structure changes (from toolbar checkbox) */
  onPreserveStructureChange?: (value: boolean) => void;

  /**
   * Enable editing: node selection, properties panel, context menus, undo/redo.
   * Default: false
   */
  editable?: boolean;

}
