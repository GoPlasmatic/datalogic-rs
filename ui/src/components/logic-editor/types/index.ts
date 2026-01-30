export type {
  JsonLogicPrimitive,
  JsonLogicVar,
  JsonLogicVal,
  JsonLogicExpression,
  JsonLogicValue,
  OperatorCategory,
  NodeCategory,
} from './jsonlogic';
export type {
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
  NodeEvaluationResult,
  EvaluationResultsMap,
  LogicEditorProps,
  DataLogicEditorProps,
} from './editor';
export type {
  ExpressionNode,
  ExecutionStep,
  TracedResult,
} from './trace';

// Re-export CATEGORY_COLORS from constants for backward compatibility
export { CATEGORY_COLORS } from '../constants/colors';
