// Scoped theme CSS variables (required for component styling)
// Uses .logic-editor scope to avoid leaking into consumer apps
import './components/logic-editor/styles/theme.css';

// Main component
export { DataLogicEditor } from './components/logic-editor/DataLogicEditor';

// Types
export type {
  // Public props
  DataLogicEditorProps,
  DataLogicEvaluationConfig,
  DataLogicCustomOperator,
  // JSONLogic types
  JsonLogicValue,
  // Node types (for advanced use)
  LogicNode,
  LogicEdge,
  LogicNodeData,
  OperatorNodeData,
  VariableNodeData,
  LiteralNodeData,
  StructureNodeData,
  StructureElement,
  CellData,
  ConversionResult,
  StructuredError,
  TracedResult,
  // Operator types
  OperatorCategory,
} from './components/logic-editor/types';
export type { FlowDirection } from './components/logic-editor/context';
export type { IconName } from './components/logic-editor/utils/icons';
export type { JsonLogicToNodesOptions } from './components/logic-editor/utils';

// Constants (for customization)
export { CATEGORY_COLORS } from './components/logic-editor/constants';
export { operators as OPERATORS } from './components/logic-editor/config/operators';

// Utilities (for advanced use)
export { jsonLogicToNodes, applyTreeLayout } from './components/logic-editor/utils';
export {
  useWasmEvaluator,
  DataLogicEvaluationError,
  summarizeEvaluationConfig,
  isDefaultEvaluationConfig,
} from './components/logic-editor/hooks';
