// Main component
export { DataLogicEditor } from './components/logic-editor/DataLogicEditor';

// Types
export type {
  // Public props
  DataLogicEditorProps,
  DataLogicEditorMode,
  // JSONLogic types
  JsonLogicValue,
  // Node types (for advanced use)
  LogicNode,
  LogicEdge,
  LogicNodeData,
  OperatorNodeData,
  VariableNodeData,
  LiteralNodeData,
  VerticalCellNodeData,
  DecisionNodeData,
  // Evaluation types
  NodeEvaluationResult,
  EvaluationResultsMap,
  // Operator types
  OperatorCategory,
} from './components/logic-editor/types';

// Constants (for customization)
export { OPERATORS, CATEGORY_COLORS } from './components/logic-editor/constants';

// Utilities (for advanced use)
export { jsonLogicToNodes, applyTreeLayout } from './components/logic-editor/utils';
