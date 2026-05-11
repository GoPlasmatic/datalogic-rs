import { v4 as uuidv4 } from 'uuid';
import type {
  JsonLogicValue,
  LogicNode,
  LogicEdge,
  LiteralNodeData,
  OperatorNodeData,
  OperatorCategory,
  CellData,
} from '../types';
import type { IconName } from './icons';
import type { ParentInfo } from './converters/types';
import { getValueType } from './type-helpers';
import { formatOperandLabel } from './formatting';


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

// Options for building variable operator cells
export interface BuildVariableCellsOptions {
  operator: 'var' | 'val' | 'exists';
  path: string;
  defaultValue?: JsonLogicValue;
  scopeJump?: number;
  pathComponents?: string[];
}

// Build cells for variable operators (var, val, exists)
export function buildVariableCells(options: BuildVariableCellsOptions): CellData[] {
  const { operator, path, defaultValue, scopeJump, pathComponents } = options;
  const cells: CellData[] = [];

  if (operator === 'var') {
    cells.push({
      type: 'editable',
      rowLabel: 'Path',
      icon: 'type',
      fieldId: 'path',
      fieldType: 'text',
      value: path,
      placeholder: 'user.profile.name',
      index: 0,
    });
    if (defaultValue !== undefined) {
      cells.push({
        type: 'inline',
        rowLabel: 'Default',
        icon: 'hash',
        label: formatOperandLabel(defaultValue),
        index: 1,
      });
    }
  } else if (operator === 'val') {
    const scope = scopeJump ?? 0;
    cells.push({
      type: 'editable',
      rowLabel: 'Scope',
      icon: 'arrow-up',
      fieldId: 'scopeLevel',
      fieldType: 'number',
      value: scope,
      label: `${scope} level${scope !== 1 ? 's' : ''} up`,
      index: 0,
    });
    cells.push({
      type: 'editable',
      rowLabel: 'Path',
      icon: 'type',
      fieldId: 'path',
      fieldType: 'text',
      value: pathComponents?.join('.') ?? path,
      placeholder: 'field1.field2',
      index: 1,
    });
  } else if (operator === 'exists') {
    cells.push({
      type: 'editable',
      rowLabel: 'Path',
      icon: 'type',
      fieldId: 'path',
      fieldType: 'text',
      value: path,
      placeholder: 'user.profile.name',
      index: 0,
    });
  }

  return cells;
}

// Factory function to create a variable node (now creates unified operator node)
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
  const cells = buildVariableCells({ operator, path, defaultValue, scopeJump, pathComponents });

  return {
    id: nodeId,
    type: 'operator',
    position: { x: 0, y: 0 },
    data: {
      type: 'operator',
      operator,
      category: 'variable' as OperatorCategory,
      label: operator === 'var' ? 'Variable' : operator === 'val' ? 'Value' : 'Exists',
      icon: 'box' as IconName,
      cells,
      collapsed: false,
      expression: originalExpr,
      parentId: parentInfo.parentId,
      argIndex: parentInfo.argIndex,
      branchType: parentInfo.branchType,
    } as OperatorNodeData,
  };
}

// Options for creating operator nodes
interface OperatorNodeOptions {
  operator: string;
  category: OperatorCategory;
  label: string;
  icon: IconName;
  cells: CellData[];
  collapsed?: boolean;
  expressionText?: string;
  expression: JsonLogicValue;
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
      icon: options.icon,
      cells: options.cells,
      collapsed: options.collapsed ?? false,
      expressionText: options.expressionText,
      expression: options.expression,
      parentId: parentInfo.parentId,
      argIndex: parentInfo.argIndex,
      branchType: parentInfo.branchType,
    } as OperatorNodeData,
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
