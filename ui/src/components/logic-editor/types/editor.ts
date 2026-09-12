import type { Node, Edge } from '@xyflow/react';
import type { OperatorCategory, JsonLogicValue } from './jsonlogic';
import type { IconName } from '../utils/icons';

// Visual node types
export type VisualNodeType = 'operator' | 'literal' | 'structure';

// Base data for all visual nodes
// Note: Index signature is required for React Flow Node type compatibility
export interface BaseNodeData extends Record<string, unknown> {
  type: VisualNodeType;
  parentId?: string;
  argIndex?: number;
  branchType?: 'yes' | 'no' | 'branch' | 'condition'; // For decision tree branches
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

/**
 * Engine evaluation settings, mirroring the WASM `Engine` config keys
 * (`EvaluationConfig::from_json_str` in the Rust crate). Every key is
 * optional; omitted keys keep the engine default (or the selected preset's
 * value). Unknown keys or values are rejected by the engine with a
 * `ConfigurationError`.
 */
export interface DataLogicEvaluationConfig {
  /** Starting point the other keys override. Default: `'default'`. */
  preset?: 'default' | 'safe_arithmetic' | 'strict';
  /** What arithmetic does with a non-numeric operand. Default: `'throw_error'`. */
  arithmetic_nan_handling?: 'throw_error' | 'ignore_value' | 'coerce_to_zero' | 'return_null';
  /**
   * What a fractional dividend over zero yields. Default: `'return_saturated'`.
   * Integer division by zero always throws `{"type": "NaN"}`.
   */
  division_by_zero?: 'return_saturated' | 'throw_error' | 'return_null' | 'return_infinity';
  /** Whether loose `==` raises on incompatible types. Default: `true`. */
  loose_equality_errors?: boolean;
  /** Truthiness rules used by `if`, `and`, `or`, `!`. Default: `'javascript'`. */
  truthy_evaluator?: 'javascript' | 'python' | 'strict_boolean';
  /** Numeric coercion knobs (all default to `true` except `reject_non_numeric`). */
  numeric_coercion?: {
    empty_string_to_zero?: boolean;
    null_to_zero?: boolean;
    bool_to_number?: boolean;
    /** When `true`, overrides every other coercion flag and raises instead. */
    reject_non_numeric?: boolean;
  };
  /** Nested evaluation-boundary cap (custom operators re-entering the engine). Default: `256`. */
  max_recursion_depth?: number;
  /**
   * Ceiling on the operations one evaluation may charge, or omitted for
   * unbounded (the default).
   *
   * One operation is charged per dispatched node, one per item an
   * iterator examines, and whatever an operator charges for the data it
   * moves. Literals and constant-folded subtrees cost nothing. Crossing
   * the ceiling raises a `BudgetExceeded` error that `try` cannot
   * recover from — the evaluation is refused before the work, not
   * reported after it.
   */
  ops_budget?: number;
}

/**
 * A custom operator implementation. Receives the already-evaluated
 * arguments and returns any JSON-serializable value (`undefined` becomes
 * `null`). A thrown exception surfaces as a runtime evaluation error.
 */
export type DataLogicCustomOperator = (args: unknown[]) => unknown;

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
   * Enable templating mode: multi-key objects and arrays in compiled rules
   * become output-shaping templates with embedded JSONLogic expressions,
   * rather than rejected as invalid JSONLogic. Matches the v5 core API
   * (`Engine::builder().with_templating(true)`).
   */
  templating?: boolean;

  /** Callback when templating mode changes (from toolbar checkbox) */
  onTemplatingChange?: (value: boolean) => void;

  /**
   * Engine evaluation settings (presets, NaN and division-by-zero handling,
   * truthiness rules, numeric coercion, recursion cap). Applied to both the
   * plain and the traced evaluation paths; the toolbar shows a compact
   * summary whenever the settings differ from the engine defaults.
   */
  config?: DataLogicEvaluationConfig;

  /**
   * Custom operators registered on the evaluation engine, keyed by operator
   * name. Rules that use them evaluate and trace normally; the palette and
   * help panel only know built-in operators, so custom nodes render with
   * the generic "utility" styling.
   */
  customOperators?: Record<string, DataLogicCustomOperator>;

  /**
   * Enable editing: node selection, properties panel, context menus, undo/redo.
   * Default: false
   */
  editable?: boolean;

  /**
   * Optional list of example names to surface as quick-action chips in the
   * empty state. Each chip, when clicked, calls `onSelectExample` with the
   * corresponding name. Ignored when the editor is non-empty.
   */
  exampleSuggestions?: string[];

  /**
   * Callback invoked when a user clicks an empty-state example chip.
   * Receives the example name from `exampleSuggestions`.
   */
  onSelectExample?: (name: string) => void;
}
