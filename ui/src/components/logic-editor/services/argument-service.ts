/**
 * Argument Service
 *
 * Provides pure functions for adding and removing arguments from operator nodes.
 */

import { v4 as uuidv4 } from 'uuid';
import type {
  LogicNode,
  OperatorNodeData,
  LiteralNodeData,
  JsonLogicValue,
  CellData,
} from '../types';
import { getOperator } from '../config/operators';
import { deleteNodeAndDescendants } from '../utils/node-deletion';
import { createArgumentNode } from './node-creation-service';

/**
 * Result of adding an argument
 */
export interface AddArgumentResult {
  nodes: LogicNode[];
  newNodeId: string;
}

/**
 * Add an argument to an operator node (unified cells-based logic)
 */
export function addArgument(
  nodes: LogicNode[],
  parentId: string,
  nodeType: 'literal' | 'variable' | 'operator',
  operatorName?: string
): AddArgumentResult | null {
  const parentNode = nodes.find((n) => n.id === parentId);
  if (!parentNode) return null;

  const parentData = parentNode.data;

  if (parentData.type !== 'operator') return null;

  const operatorData = parentData as OperatorNodeData;
  const opConfig = getOperator(operatorData.operator);

  if (!opConfig) return null;

  // Allow adding for nary, variadic, chainable, special, and range arity types
  const { arity } = opConfig;
  const allowAdd = arity.type === 'nary' || arity.type === 'variadic' ||
    arity.type === 'chainable' || arity.type === 'special' || arity.type === 'range';

  if (!allowAdd) return null;

  if (arity.max && operatorData.cells.length >= arity.max) {
    return null;
  }

  // Special handling for if operator: add condition+then pair
  if (operatorData.operator === 'if' || operatorData.operator === '?:') {
    return addIfElsePair(nodes, parentNode, operatorData);
  }

  // Special handling for val operator: add editable path component cell
  if (operatorData.operator === 'val') {
    return addValPathCell(nodes, parentNode, operatorData);
  }

  const expr = operatorData.expression;
  let currentOperands: JsonLogicValue[] = [];
  if (expr && typeof expr === 'object' && !Array.isArray(expr)) {
    const opKey = Object.keys(expr)[0];
    const operands = (expr as Record<string, unknown>)[opKey];
    currentOperands = Array.isArray(operands) ? operands : [operands as JsonLogicValue];
  }

  const newIndex = currentOperands.length;
  const newNodes = createArgumentNode(nodeType, parentId, newIndex, opConfig.category, operatorName);
  const newNodeId = newNodes[0].id;

  const newNodeData = newNodes[0].data;
  let newValue: JsonLogicValue = 0;
  if (newNodeData.type === 'literal') {
    newValue = (newNodeData as LiteralNodeData).value as JsonLogicValue;
  } else if (newNodeData.type === 'operator') {
    newValue = (newNodeData as OperatorNodeData).expression as JsonLogicValue;
  }

  const newOperands = [...currentOperands, newValue];
  const newExpression = { [operatorData.operator]: newOperands } as JsonLogicValue;

  const updatedParent: LogicNode = {
    ...parentNode,
    data: {
      ...operatorData,
      cells: [
        ...operatorData.cells,
        {
          type: 'branch' as const,
          branchId: newNodeId,
          index: newIndex,
        },
      ],
      expression: newExpression,
      expressionText: undefined,
    },
  };

  const result = nodes.map((n) => (n.id === parentId ? updatedParent : n));
  result.push(...newNodes);

  return { nodes: result, newNodeId };
}

/**
 * Add an Else If condition+then pair to an if operator.
 * Inserts before the final Else cell (if exists).
 */
function addIfElsePair(
  nodes: LogicNode[],
  parentNode: LogicNode,
  operatorData: OperatorNodeData
): AddArgumentResult {
  const conditionId = uuidv4();
  const thenId = uuidv4();

  // Create condition node (literal true by default)
  const conditionNode: LogicNode = {
    id: conditionId,
    type: 'literal',
    position: { x: 0, y: 0 },
    data: {
      type: 'literal',
      value: true,
      valueType: 'boolean',
      expression: true,
      parentId: parentNode.id,
      argIndex: 0, // Will be set below
    } as LiteralNodeData,
  };

  // Create then node (literal 0 by default)
  const thenNode: LogicNode = {
    id: thenId,
    type: 'literal',
    position: { x: 0, y: 0 },
    data: {
      type: 'literal',
      value: 0,
      valueType: 'number',
      expression: 0,
      parentId: parentNode.id,
      argIndex: 0, // Will be set below
    } as LiteralNodeData,
  };

  // Determine insertion point: before the Else cell if it exists
  const cells = [...operatorData.cells];
  const hasElse = cells.length > 0 && cells[cells.length - 1].rowLabel === 'Else';
  const insertIndex = hasElse ? cells.length - 1 : cells.length;

  // Build the new condition cell and then cell
  const conditionCell: CellData = {
    type: 'branch',
    icon: 'diamond',
    rowLabel: 'Else If',
    branchId: conditionId,
    index: insertIndex,
  };
  const thenCell: CellData = {
    type: 'branch',
    icon: 'check',
    rowLabel: 'Then',
    branchId: thenId,
    index: insertIndex + 1,
  };

  // Insert the pair and reindex
  cells.splice(insertIndex, 0, conditionCell, thenCell);
  const reindexedCells = cells.map((c, i) => ({ ...c, index: i }));

  // Rebuild expression operands
  const expr = operatorData.expression;
  let currentOperands: JsonLogicValue[] = [];
  if (expr && typeof expr === 'object' && !Array.isArray(expr)) {
    const opKey = Object.keys(expr)[0];
    const operands = (expr as Record<string, unknown>)[opKey];
    currentOperands = Array.isArray(operands) ? operands : [operands as JsonLogicValue];
  }

  // Insert condition (true) and then (0) at the expression level
  // The expression operands map: [cond, then, cond, then, ..., else?]
  // Insert point in operands is the same as insertIndex in cells
  const newOperands = [...currentOperands];
  newOperands.splice(insertIndex, 0, true, 0);
  const newExpression = { [operatorData.operator]: newOperands } as JsonLogicValue;

  // Update argIndex on condition and then nodes
  conditionNode.data.argIndex = insertIndex;
  thenNode.data.argIndex = insertIndex + 1;

  const updatedParent: LogicNode = {
    ...parentNode,
    data: {
      ...operatorData,
      cells: reindexedCells,
      expression: newExpression,
      expressionText: undefined,
    },
  };

  // Reindex argIndex on existing children that shifted
  const result = nodes.map((n) => {
    if (n.id === parentNode.id) return updatedParent;
    if (n.data.parentId === parentNode.id && (n.data.argIndex ?? 0) >= insertIndex) {
      return {
        ...n,
        data: {
          ...n.data,
          argIndex: (n.data.argIndex ?? 0) + 2,
        },
      };
    }
    return n;
  });
  result.push(conditionNode, thenNode);

  return { nodes: result, newNodeId: conditionId };
}

/**
 * Add an editable path component cell to a val operator.
 */
function addValPathCell(
  nodes: LogicNode[],
  parentNode: LogicNode,
  operatorData: OperatorNodeData
): AddArgumentResult {
  const newIndex = operatorData.cells.length;

  const newCell: CellData = {
    type: 'editable',
    rowLabel: 'Path',
    icon: 'type',
    fieldId: 'path',
    fieldType: 'text',
    value: '',
    placeholder: 'field.name',
    index: newIndex,
  };

  // Rebuild expression from current cells + new cell
  const newCells = [...operatorData.cells, newCell];
  const newExpression = rebuildValExpression(newCells);

  const updatedParent: LogicNode = {
    ...parentNode,
    data: {
      ...operatorData,
      cells: newCells,
      expression: newExpression,
      expressionText: undefined,
    },
  };

  const result = nodes.map((n) => (n.id === parentNode.id ? updatedParent : n));
  return { nodes: result, newNodeId: parentNode.id };
}

/**
 * Rebuild val expression from cells.
 */
function rebuildValExpression(cells: CellData[]): JsonLogicValue {
  const scopeCell = cells.find((c) => c.fieldId === 'scopeLevel');
  const pathCells = cells.filter((c) => c.fieldId === 'path');
  const scopeJump = typeof scopeCell?.value === 'number' ? scopeCell.value : 0;

  const pathComponents: string[] = [];
  for (const pc of pathCells) {
    const pathStr = String(pc.value ?? '');
    if (pathStr) {
      pathStr.split('.').forEach((comp) => {
        if (comp) pathComponents.push(comp);
      });
    }
  }

  if (scopeJump === 0 && pathComponents.length === 1 &&
      (pathComponents[0] === 'index' || pathComponents[0] === 'key')) {
    return { val: pathComponents[0] };
  }

  const args: JsonLogicValue[] = [];
  if (scopeJump > 0) {
    args.push([-scopeJump]);
  }
  args.push(...pathComponents);
  return { val: args.length === 0 ? [] : args };
}

/**
 * Remove an argument from an operator node (unified cells-based logic)
 */
export function removeArgument(
  nodes: LogicNode[],
  parentId: string,
  argIndex: number
): LogicNode[] | null {
  const parentNode = nodes.find((n) => n.id === parentId);
  if (!parentNode) return null;

  const parentData = parentNode.data;

  if (parentData.type !== 'operator') return null;

  const operatorData = parentData as OperatorNodeData;
  const opConfig = getOperator(operatorData.operator);

  const minArgs = opConfig?.arity.min ?? 0;
  if (operatorData.cells.length <= minArgs) {
    return null;
  }

  // Special handling for if operator: remove condition+then pair
  if (operatorData.operator === 'if' || operatorData.operator === '?:') {
    return removeIfElsePair(nodes, parentNode, operatorData, argIndex);
  }

  const cellToRemove = operatorData.cells.find((c) => c.index === argIndex);
  if (!cellToRemove) return null;

  const expr = operatorData.expression;
  let currentOperands: JsonLogicValue[] = [];
  if (expr && typeof expr === 'object' && !Array.isArray(expr)) {
    const opKey = Object.keys(expr)[0];
    const operands = (expr as Record<string, unknown>)[opKey];
    currentOperands = Array.isArray(operands) ? operands : [operands as JsonLogicValue];
  }

  let newNodes = cellToRemove.branchId
    ? deleteNodeAndDescendants(cellToRemove.branchId, nodes)
    : nodes;

  const newOperands = currentOperands.filter((_, i) => i !== argIndex);
  const newExpression = { [operatorData.operator]: newOperands } as JsonLogicValue;

  newNodes = newNodes.map((n) => {
    if (n.id === parentId) {
      const updatedCells = operatorData.cells
        .filter((c) => c.index !== argIndex)
        .map((c) => ({
          ...c,
          index: c.index > argIndex ? c.index - 1 : c.index,
        }));
      return {
        ...n,
        data: {
          ...operatorData,
          cells: updatedCells,
          expression: newExpression,
          expressionText: undefined,
        },
      };
    }
    if (n.data.parentId === parentId && (n.data.argIndex ?? 0) > argIndex) {
      return {
        ...n,
        data: {
          ...n.data,
          argIndex: (n.data.argIndex ?? 0) - 1,
        },
      };
    }
    return n;
  });

  return newNodes;
}

/**
 * Remove an Else If condition+then pair (or the Else) from an if operator.
 * argIndex is the "pair index" (0 = first if/then, 1 = second else-if/then, etc.)
 * or the special else index.
 *
 * We use the cell's rowLabel to determine what to remove:
 * - 'If' or 'Else If' row: remove condition + following Then cell (2 cells)
 * - 'Else' row: remove just the Else cell (1 cell)
 */
function removeIfElsePair(
  nodes: LogicNode[],
  parentNode: LogicNode,
  operatorData: OperatorNodeData,
  argIndex: number
): LogicNode[] | null {
  const cell = operatorData.cells.find((c) => c.index === argIndex);
  if (!cell) return null;

  let cellIndicesToRemove: number[];

  if (cell.rowLabel === 'If' || cell.rowLabel === 'Else If') {
    // Remove condition + the following Then cell
    cellIndicesToRemove = [argIndex, argIndex + 1];
  } else if (cell.rowLabel === 'Then') {
    // User clicked on the Then cell — remove its paired condition + this Then
    cellIndicesToRemove = [argIndex - 1, argIndex];
  } else if (cell.rowLabel === 'Else') {
    // Remove just the Else cell
    cellIndicesToRemove = [argIndex];
  } else {
    return null;
  }

  // Don't remove the last If/Then pair (must keep at least one condition+then)
  const conditionCells = operatorData.cells.filter(
    (c) => c.rowLabel === 'If' || c.rowLabel === 'Else If'
  );
  const removingConditions = cellIndicesToRemove.filter((idx) => {
    const c = operatorData.cells.find((cell) => cell.index === idx);
    return c?.rowLabel === 'If' || c?.rowLabel === 'Else If';
  });
  if (conditionCells.length - removingConditions.length < 1) {
    return null;
  }

  // Delete child nodes for removed cells
  let newNodes = [...nodes];
  for (const idx of cellIndicesToRemove) {
    const cellToRemove = operatorData.cells.find((c) => c.index === idx);
    if (cellToRemove?.branchId) {
      newNodes = deleteNodeAndDescendants(cellToRemove.branchId, newNodes);
    }
  }

  // Remove operands from expression
  const expr = operatorData.expression;
  let currentOperands: JsonLogicValue[] = [];
  if (expr && typeof expr === 'object' && !Array.isArray(expr)) {
    const opKey = Object.keys(expr)[0];
    const operands = (expr as Record<string, unknown>)[opKey];
    currentOperands = Array.isArray(operands) ? operands : [operands as JsonLogicValue];
  }

  const removeSet = new Set(cellIndicesToRemove);
  const newOperands = currentOperands.filter((_, i) => !removeSet.has(i));
  const newExpression = { [operatorData.operator]: newOperands } as JsonLogicValue;

  // Remove cells and reindex
  const removedCount = cellIndicesToRemove.length;
  const minRemovedIndex = Math.min(...cellIndicesToRemove);

  // Update the first remaining condition to be "If" if we removed the original "If"
  const updatedCells = operatorData.cells
    .filter((c) => !removeSet.has(c.index))
    .map((c, i) => ({
      ...c,
      index: i,
    }));

  // Ensure the first condition cell is labeled "If" (not "Else If")
  if (updatedCells.length > 0 && updatedCells[0].rowLabel === 'Else If') {
    updatedCells[0] = { ...updatedCells[0], rowLabel: 'If' };
  }

  newNodes = newNodes.map((n) => {
    if (n.id === parentNode.id) {
      return {
        ...n,
        data: {
          ...operatorData,
          cells: updatedCells,
          expression: newExpression,
          expressionText: undefined,
        },
      };
    }
    if (n.data.parentId === parentNode.id && (n.data.argIndex ?? 0) > minRemovedIndex) {
      return {
        ...n,
        data: {
          ...n.data,
          argIndex: Math.max(0, (n.data.argIndex ?? 0) - removedCount),
        },
      };
    }
    return n;
  });

  return newNodes;
}
