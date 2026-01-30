// Theme CSS variables (required for component styling)
import './index.css';

// Main component
export { DataLogicEditor } from './components/logic-editor/DataLogicEditor';

// Types
export type {
  // Public props
  DataLogicEditorProps,
  // JSONLogic types
  JsonLogicValue,
  // Node types (for advanced use)
  LogicNode,
  LogicEdge,
  LogicNodeData,
  OperatorNodeData,
  VariableNodeData,
  LiteralNodeData,
  // Evaluation types
  NodeEvaluationResult,
  EvaluationResultsMap,
  // Operator types
  OperatorCategory,
} from './components/logic-editor/types';

// Constants (for customization)
export { CATEGORY_COLORS } from './components/logic-editor/constants';
export { operators as OPERATORS } from './components/logic-editor/config/operators';

// Utilities (for advanced use)
export { jsonLogicToNodes, applyTreeLayout } from './components/logic-editor/utils';
