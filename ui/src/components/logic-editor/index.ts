// Main component
export { DataLogicEditor } from './DataLogicEditor';

// Types
export type {
  DataLogicEditorProps,
  DataLogicEvaluationConfig,
  DataLogicCustomOperator,
} from './types';
export type {
  JsonLogicPrimitive,
  JsonLogicVar,
  JsonLogicVal,
  JsonLogicExpression,
  JsonLogicValue,
  OperatorCategory,
  NodeCategory,
  VisualNodeType,
  BaseNodeData,
  ArgSummary,
  OperatorNodeData,
  LiteralNodeData,
  CellData,
  VariableNodeData,
  StructureElement,
  StructureNodeData,
  LogicNodeData,
  LogicNode,
  LogicEdge,
  EditorState,
  ConversionResult,
  ExpressionNode,
  ExecutionStep,
  TracedResult,
  StructuredError,
} from './types';
export { CATEGORY_COLORS } from './types';

// Constants
export {
  TRUNCATION_LIMITS,
  BRANCH_COLORS,
  NODE_DIMENSIONS,
  VERTICAL_CELL_DIMENSIONS,
  TEXT_METRICS,
  NODE_PADDING,
  DAGRE_OPTIONS,
  FIXED_WIDTHS,
  DECISION_NODE_DIMENSIONS,
  REACT_FLOW_OPTIONS,
  HANDLE_IDS,
  HANDLE_POSITIONS,
  EDGE_IDS,
} from './constants';

// Utilities
export {
  jsonLogicToNodes,
  type JsonLogicToNodesOptions,
  traceToNodes,
  applyTreeLayout,
  getHiddenNodeIds,
  isOperatorNode,
  isLiteralNode,
  isStructureNode,
  isCollapsibleNode,
  getOperatorNodeData,
  createLiteralNode,
  buildVariableCells,
  createVariableNode,
  createOperatorNode,
  createEdge,
  createArgEdge,
  createBranchEdge,
  panelValuesToNodeData,
  havePanelValuesChanged,
  deleteNodeAndDescendants,
  getDescendantIds,
  isRootNode,
  canDeleteNode,
  nodesToJsonLogic,
  getRootNode,
  cloneNodesWithIdMapping,
  getDescendants,
  updateParentChildReference,
  capitalizeFirst,
  buildOperatorSubmenu,
} from './utils';
