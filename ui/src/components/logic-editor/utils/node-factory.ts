import { v4 as uuidv4 } from 'uuid';
import type {
  JsonLogicValue,
  LogicNode,
  LogicEdge,
  LiteralNodeData,
  OperatorNodeData,
  VariableNodeData,
  VerticalCellNodeData,
  DecisionNodeData,
  OperatorCategory,
  CellData,
} from '../types';
import type { IconName } from './icons';
import type { ParentInfo } from './converters/types';
import { getValueType } from './type-helpers';
import { BRANCH_COLORS } from '../constants';

// Factory function to create a literal node
export function createLiteralNode(
  value: JsonLogicValue,
  parentInfo: ParentInfo = {}
): LogicNode {
  const nodeId = uuidv4();
  return {
    id: nodeId,
    type: 'literal',
    position: { x: 0, y: 0 },
    data: {
      type: 'literal',
      value,
      valueType: getValueType(value),
      expression: value,
      parentId: parentInfo.parentId,
      argIndex: parentInfo.argIndex,
      branchType: parentInfo.branchType,
    } as LiteralNodeData,
  };
}

// Factory function to create a variable node
export function createVariableNode(
  operator: 'var' | 'val' | 'exists',
  path: string,
  defaultValue: JsonLogicValue | undefined,
  originalExpr: JsonLogicValue,
  parentInfo: ParentInfo = {},
  scopeJump?: number,
  pathComponents?: string[]
): LogicNode {
  const nodeId = uuidv4();
  return {
    id: nodeId,
    type: 'variable',
    position: { x: 0, y: 0 },
    data: {
      type: 'variable',
      operator,
      path,
      defaultValue,
      scopeJump,
      pathComponents,
      expression: originalExpr,
      parentId: parentInfo.parentId,
      argIndex: parentInfo.argIndex,
      branchType: parentInfo.branchType,
    } as VariableNodeData,
  };
}

// Options for creating operator nodes
interface OperatorNodeOptions {
  operator: string;
  category: OperatorCategory;
  label: string;
  childIds: string[];
  collapsed?: boolean;
  expressionText?: string;
  expression: JsonLogicValue;
  inlineDisplay?: string;
}

// Factory function to create an operator node
export function createOperatorNode(
  options: OperatorNodeOptions,
  parentInfo: ParentInfo = {}
): LogicNode {
  const nodeId = uuidv4();
  return {
    id: nodeId,
    type: 'operator',
    position: { x: 0, y: 0 },
    data: {
      type: 'operator',
      operator: options.operator,
      category: options.category,
      label: options.label,
      childIds: options.childIds,
      collapsed: options.collapsed ?? false,
      expressionText: options.expressionText,
      expression: options.expression,
      inlineDisplay: options.inlineDisplay,
      parentId: parentInfo.parentId,
      argIndex: parentInfo.argIndex,
      branchType: parentInfo.branchType,
    } as OperatorNodeData,
  };
}

// Options for creating vertical cell nodes
interface VerticalCellNodeOptions {
  operator: string;
  category: OperatorCategory;
  label: string;
  icon: IconName;
  cells: CellData[];
  collapsed?: boolean;
  expressionText?: string;
  expression: JsonLogicValue;
}

// Factory function to create a vertical cell node
export function createVerticalCellNode(
  options: VerticalCellNodeOptions,
  parentInfo: ParentInfo = {}
): LogicNode {
  const nodeId = uuidv4();
  return {
    id: nodeId,
    type: 'verticalCell',
    position: { x: 0, y: 0 },
    data: {
      type: 'verticalCell',
      operator: options.operator,
      category: options.category,
      label: options.label,
      icon: options.icon,
      cells: options.cells,
      collapsed: options.collapsed ?? false,
      expressionText: options.expressionText,
      expression: options.expression,
      parentId: parentInfo.parentId,
      argIndex: parentInfo.argIndex,
      branchType: parentInfo.branchType,
    } as VerticalCellNodeData,
  };
}

// Edge creation options
interface EdgeOptions {
  source: string;
  target: string;
  sourceHandle?: string;
  targetHandle?: string;
  label?: string;
  className?: string;
  style?: Record<string, string>;
}

// Factory function to create an edge
export function createEdge(options: EdgeOptions): LogicEdge {
  const edge: LogicEdge = {
    id: `${options.source}-${options.target}`,
    source: options.source,
    target: options.target,
  };

  if (options.sourceHandle) {
    edge.sourceHandle = options.sourceHandle;
  }
  if (options.targetHandle) {
    edge.targetHandle = options.targetHandle;
  }
  if (options.label) {
    edge.label = options.label;
  }
  if (options.className) {
    edge.className = options.className;
  }
  if (options.style) {
    edge.style = options.style;
  }

  return edge;
}

// Create edge from parent to child using argument index
export function createArgEdge(parentId: string, childId: string, argIndex: number): LogicEdge {
  return createEdge({
    source: parentId,
    target: childId,
    sourceHandle: `arg-${argIndex}`,
    targetHandle: 'left',
  });
}

// Create edge from parent to branch using branch index
export function createBranchEdge(
  parentId: string,
  branchId: string,
  branchIndex: number,
  options: { label?: string; className?: string; style?: Record<string, string> } = {}
): LogicEdge {
  return createEdge({
    source: parentId,
    target: branchId,
    sourceHandle: `branch-${branchIndex}`,
    targetHandle: 'left',
    ...options,
  });
}

// Options for creating decision nodes
interface DecisionNodeOptions {
  conditionText: string;
  conditionExpression: JsonLogicValue;
  isConditionComplex: boolean;
  conditionBranchId?: string;
  yesBranchId: string;
  noBranchId: string;
  collapsed?: boolean;
  expressionText?: string;
  expression: JsonLogicValue;
}

// Factory function to create a decision node
export function createDecisionNode(
  options: DecisionNodeOptions,
  parentInfo: ParentInfo = {}
): LogicNode {
  const nodeId = uuidv4();
  return {
    id: nodeId,
    type: 'decision',
    position: { x: 0, y: 0 },
    data: {
      type: 'decision',
      conditionText: options.conditionText,
      conditionExpression: options.conditionExpression,
      isConditionComplex: options.isConditionComplex,
      conditionBranchId: options.conditionBranchId,
      yesBranchId: options.yesBranchId,
      noBranchId: options.noBranchId,
      collapsed: options.collapsed ?? false,
      expressionText: options.expressionText,
      expression: options.expression,
      parentId: parentInfo.parentId,
      argIndex: parentInfo.argIndex,
      branchType: parentInfo.branchType,
    } as DecisionNodeData,
  };
}

// Create "Yes" branch edge from decision node
// branchIndex: 0 if no condition branch, 1 if condition branch exists
export function createYesEdge(parentId: string, childId: string, hasConditionBranch: boolean): LogicEdge {
  const branchIndex = hasConditionBranch ? 1 : 0;
  return {
    id: `${parentId}-yes-${childId}`,
    source: parentId,
    target: childId,
    sourceHandle: `branch-${branchIndex}`,
    targetHandle: 'left',
    label: 'Yes',
    className: 'yes-edge',
    style: { stroke: BRANCH_COLORS.yes },
  };
}

// Create "No" branch edge from decision node
// branchIndex: 1 if no condition branch, 2 if condition branch exists
export function createNoEdge(parentId: string, childId: string, hasConditionBranch: boolean): LogicEdge {
  const branchIndex = hasConditionBranch ? 2 : 1;
  return {
    id: `${parentId}-no-${childId}`,
    source: parentId,
    target: childId,
    sourceHandle: `branch-${branchIndex}`,
    targetHandle: 'left',
    label: 'No',
    className: 'no-edge',
    style: { stroke: BRANCH_COLORS.no },
  };
}

// Create condition branch edge from decision node (for complex conditions)
// Always branch-0 when it exists
export function createConditionEdge(parentId: string, childId: string): LogicEdge {
  return {
    id: `${parentId}-cond-${childId}`,
    source: parentId,
    target: childId,
    sourceHandle: 'branch-0',
    targetHandle: 'left',
  };
}
