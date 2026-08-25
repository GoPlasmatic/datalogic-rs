/**
 * Argument Service
 *
 * Provides pure functions for adding and removing arguments from operator nodes.
 *
 * The service is parent-shape aware:
 * - n-ary operators append / drop an operand
 * - decision diamonds (if / ?:) add an else-if as a new diamond chained into
 *   the else input, and remove either the else input or a whole diamond
 * - switch/match add or remove a Case/Then pair (or the Default row)
 * - val adds / removes a path component cell; var drops its default
 * - exists takes exactly one path, so nothing can be added
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
import { replaceChildReference } from '../utils/node-cloning';
import { formatOperandLabel } from '../utils/formatting';
import {
  isIfOperator,
  isDecisionCells,
  decisionCell,
  decisionSlotOf,
  makeDecisionBranchCell,
} from '../utils/converters/if-else-converter';
import {
  isSwitchOperator,
  isSwitchCells,
  resolveSwitchCellValues,
  switchArgsFromCells,
  switchPairPartner,
} from '../utils/converters/switch-cells';
import {
  hasVariableCells,
  variableCellsToExpression,
  rawOperandOf,
} from '../utils/converters/variable-cells';
import { createArgumentNode } from './node-creation-service';

/**
 * Result of adding an argument
 */
export interface AddArgumentResult {
  nodes: LogicNode[];
  newNodeId: string;
}

/** True when the operator's arity lets the editor add / remove arguments. */
export function canEditArguments(operator: string): boolean {
  const opConfig = getOperator(operator);
  if (!opConfig) return false;
  // exists takes a single path, so there is nothing to add or remove.
  if (operator === 'exists') return false;
  const { arity } = opConfig;
  return (
    arity.type === 'nary' ||
    arity.type === 'variadic' ||
    arity.type === 'chainable' ||
    arity.type === 'special' ||
    arity.type === 'range'
  );
}

/** The stored operands of a node's expression, normalized to an array. */
function storedOperandsOf(data: OperatorNodeData): JsonLogicValue[] {
  const raw = rawOperandOf(data.expression);
  if (raw === undefined) return [];
  return Array.isArray(raw) ? raw : [raw];
}

/** A literal node parented to `parentId` at `argIndex`. */
function literalNode(
  value: JsonLogicValue,
  valueType: LiteralNodeData['valueType'],
  parentId: string,
  argIndex: number,
  branchType?: LiteralNodeData['branchType']
): LogicNode {
  return {
    id: uuidv4(),
    type: 'literal',
    position: { x: 0, y: 0 },
    data: {
      type: 'literal',
      value,
      valueType,
      expression: value,
      parentId,
      argIndex,
      branchType,
    } as LiteralNodeData,
  };
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

  if (!opConfig || !canEditArguments(operatorData.operator)) return null;

  const { arity } = opConfig;
  if (arity.max && operatorData.cells.length >= arity.max) {
    return null;
  }

  // Special handling for if operator: chain a new else-if diamond
  if (isIfOperator(operatorData.operator) && isDecisionCells(operatorData.cells)) {
    return addElseIfDiamond(nodes, parentNode, operatorData);
  }

  // Special handling for switch/match: add a Case/Then pair
  if (isSwitchOperator(operatorData.operator) && isSwitchCells(operatorData.cells)) {
    return addSwitchCase(nodes, parentNode, operatorData);
  }

  // Special handling for val operator: add editable path component cell
  if (operatorData.operator === 'val' && hasVariableCells(operatorData.cells)) {
    return addValPathCell(nodes, parentNode, operatorData);
  }

  const currentOperands = storedOperandsOf(operatorData);

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
 * Add an else-if to a decision diamond: a new 'elif' diamond (condition true,
 * then 0) is wired into the else input, and the previous else value (if any)
 * becomes the new diamond's else input.
 */
function addElseIfDiamond(
  nodes: LogicNode[],
  parentNode: LogicNode,
  operatorData: OperatorNodeData
): AddArgumentResult {
  const elifId = uuidv4();
  const elseCell = decisionCell(operatorData.cells, 'else');
  const oldElseId = elseCell?.type === 'branch' ? elseCell.branchId : undefined;
  const stored = storedOperandsOf(operatorData);

  const conditionNode = literalNode(true, 'boolean', elifId, 0, 'condition');
  const thenNode = literalNode(0, 'number', elifId, 1, 'yes');

  const elifCells: CellData[] = [
    makeDecisionBranchCell('when', conditionNode.id, 'true'),
    makeDecisionBranchCell('then', thenNode.id, '0'),
  ];
  const elifArgs: JsonLogicValue[] = [true, 0];

  let oldElseExpression: JsonLogicValue | undefined;
  if (elseCell) {
    if (oldElseId) {
      const oldElseNode = nodes.find((n) => n.id === oldElseId);
      oldElseExpression = oldElseNode?.data.expression ?? stored[elseCell.index] ?? null;
      elifCells.push(makeDecisionBranchCell('else', oldElseId, elseCell.label));
    } else {
      // Inline else placeholder: keep its value on the new diamond as an inline row
      oldElseExpression = stored[elseCell.index] ?? null;
      elifCells.push({ ...elseCell, index: 2 });
    }
    elifArgs.push(oldElseExpression === undefined ? null : oldElseExpression);
  }

  const elifNode: LogicNode = {
    id: elifId,
    type: 'operator',
    position: { x: 0, y: 0 },
    data: {
      type: 'operator',
      operator: operatorData.operator,
      category: 'control',
      label: 'elif',
      icon: 'diamond',
      cells: elifCells,
      collapsed: false,
      parentId: parentNode.id,
      argIndex: 2,
      branchType: 'no',
      expression: { [operatorData.operator]: elifArgs },
    } as OperatorNodeData,
  };

  // The parent's else input now points at the new diamond
  const parentCells = operatorData.cells.filter((c) => decisionSlotOf(c) !== 'else');
  parentCells.push(makeDecisionBranchCell('else', elifId, 'else if'));
  const parentArgs: JsonLogicValue[] = [stored[0] ?? null, stored[1] ?? null, elifNode.data.expression as JsonLogicValue];

  const updatedParent: LogicNode = {
    ...parentNode,
    data: {
      ...operatorData,
      cells: parentCells,
      expression: { [operatorData.operator]: parentArgs },
      expressionText: undefined,
    },
  };

  const result = nodes.map((n) => {
    if (n.id === parentNode.id) return updatedParent;
    if (oldElseId && n.id === oldElseId) {
      return { ...n, data: { ...n.data, parentId: elifId, argIndex: 2, branchType: 'no' as const } };
    }
    return n;
  });
  result.push(elifNode, conditionNode, thenNode);

  return { nodes: result, newNodeId: elifId };
}

/**
 * Append a Case/Then pair to a switch/match node (before the Default row).
 * The case value is an inline literal 0; the result is a literal child node.
 */
function addSwitchCase(
  nodes: LogicNode[],
  parentNode: LogicNode,
  operatorData: OperatorNodeData
): AddArgumentResult {
  const stored = storedOperandsOf(operatorData);
  const values = resolveSwitchCellValues(operatorData.cells, stored);

  const defaultPos = operatorData.cells.findIndex((c) => c.rowLabel === 'Default');
  const insertAt = defaultPos === -1 ? operatorData.cells.length : defaultPos;

  const thenNode = literalNode(0, 'number', parentNode.id, insertAt + 1, 'yes');
  const caseCell: CellData = {
    type: 'inline',
    icon: 'tag',
    rowLabel: 'Case',
    label: formatOperandLabel(0),
    index: insertAt,
  };
  const thenCell: CellData = {
    type: 'branch',
    icon: 'check',
    rowLabel: 'Then',
    label: '0',
    branchId: thenNode.id,
    index: insertAt + 1,
  };

  const cells = [...operatorData.cells];
  cells.splice(insertAt, 0, caseCell, thenCell);
  // Resolved values are keyed by the OLD indices; shift the rows after the insert point
  const shifted = new Map<number, JsonLogicValue>();
  for (const [index, value] of values) {
    shifted.set(index >= insertAt ? index + 2 : index, value);
  }
  shifted.set(insertAt, 0);
  shifted.set(insertAt + 1, 0);
  const reindexed = cells.map((cell, idx) => ({ ...cell, index: idx }));
  const args = switchArgsFromCells(reindexed, shifted, true);

  const updatedParent: LogicNode = {
    ...parentNode,
    data: {
      ...operatorData,
      cells: reindexed,
      expression: { [operatorData.operator]: args },
      expressionText: undefined,
    },
  };

  const result = nodes.map((n) => {
    if (n.id === parentNode.id) return updatedParent;
    if (n.data.parentId === parentNode.id && (n.data.argIndex ?? 0) >= insertAt) {
      return { ...n, data: { ...n.data, argIndex: (n.data.argIndex ?? 0) + 2 } };
    }
    return n;
  });
  result.push(thenNode);

  return { nodes: result, newNodeId: thenNode.id };
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
    value: [],
    label: '',
    placeholder: 'field.name',
    index: newIndex,
  };

  // Rebuild expression from current cells + new cell
  const newCells = [...operatorData.cells, newCell];
  const newExpression =
    variableCellsToExpression('val', newCells, { rawOperand: rawOperandOf(operatorData.expression) }) ??
    { val: [] };

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
 * Remove an argument from an operator node (unified cells-based logic).
 * `argIndex` is the cell index of the argument to remove.
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
  if (!canEditArguments(operatorData.operator)) return null;
  const opConfig = getOperator(operatorData.operator);

  // Special handling for if operator: remove a diamond or its else input
  if (isIfOperator(operatorData.operator) && isDecisionCells(operatorData.cells)) {
    return removeDecisionInput(nodes, parentNode, operatorData, argIndex);
  }

  // Special handling for switch/match: remove a Case/Then pair or the Default
  if (isSwitchOperator(operatorData.operator) && isSwitchCells(operatorData.cells)) {
    return removeSwitchRow(nodes, parentNode, operatorData, argIndex);
  }

  const minArgs = opConfig?.arity.min ?? 0;
  if (operatorData.cells.length <= minArgs) {
    return null;
  }

  const cellToRemove = operatorData.cells.find((c) => c.index === argIndex);
  if (!cellToRemove) return null;

  // Variable operators (var default, val path components): rebuild from cells
  if (hasVariableCells(operatorData.cells)) {
    if (cellToRemove.fieldId === 'scopeLevel' || (operatorData.operator === 'var' && cellToRemove.fieldId === 'path')) {
      return null;
    }
    let newNodes = cellToRemove.branchId
      ? deleteNodeAndDescendants(cellToRemove.branchId, nodes)
      : nodes;
    const remaining = operatorData.cells
      .filter((c) => c.index !== argIndex)
      .map((c, i) => ({ ...c, index: i }));
    const expression =
      variableCellsToExpression(operatorData.operator, remaining, {
        // A var without its default reads best in the plain string form
        rawOperand: operatorData.operator === 'var' ? undefined : rawOperandOf(operatorData.expression),
      }) ?? operatorData.expression ?? null;
    newNodes = newNodes.map((n) =>
      n.id === parentId
        ? { ...n, data: { ...operatorData, cells: remaining, expression, expressionText: undefined } }
        : n
    );
    return newNodes;
  }

  const currentOperands = storedOperandsOf(operatorData);

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
 * Remove an input of a decision diamond.
 * - else: the else subtree is deleted and the else row removed
 * - when / then: the whole diamond is removed. That is only possible when an
 *   else-if diamond follows in the chain (it takes this diamond's place);
 *   the last condition of a chain cannot be removed.
 */
function removeDecisionInput(
  nodes: LogicNode[],
  parentNode: LogicNode,
  operatorData: OperatorNodeData,
  argIndex: number
): LogicNode[] | null {
  const cell = operatorData.cells.find((c) => c.index === argIndex);
  if (!cell) return null;
  const slot = decisionSlotOf(cell);
  if (!slot) return null;

  if (slot === 'else') {
    if (cell.branchId) {
      // deleteNodeAndDescendants drops the else row and rebuilds the expression
      return deleteNodeAndDescendants(cell.branchId, nodes);
    }
    const stored = storedOperandsOf(operatorData);
    const cells = operatorData.cells.filter((c) => c.index !== argIndex);
    return nodes.map((n) =>
      n.id === parentNode.id
        ? {
            ...n,
            data: {
              ...operatorData,
              cells,
              expression: { [operatorData.operator]: [stored[0] ?? null, stored[1] ?? null] },
              expressionText: undefined,
            },
          }
        : n
    );
  }

  // Removing the condition: promote the chained else-if diamond
  const elseCell = decisionCell(operatorData.cells, 'else');
  const nextId = elseCell?.branchId;
  const nextNode = nextId ? nodes.find((n) => n.id === nextId) : undefined;
  if (
    !nextNode ||
    nextNode.data.type !== 'operator' ||
    nextNode.data.label !== 'elif' ||
    !isIfOperator((nextNode.data as OperatorNodeData).operator)
  ) {
    return null;
  }

  // Delete this diamond's when/then subtrees
  let newNodes = nodes;
  for (const c of operatorData.cells) {
    if (decisionSlotOf(c) !== 'else' && c.branchId) {
      newNodes = newNodes.filter((n) => n.id !== c.branchId && !descendantOf(n, c.branchId!, nodes));
    }
  }

  // The next diamond takes this diamond's place in the tree
  const grandParentId = operatorData.parentId;
  newNodes = newNodes
    .filter((n) => n.id !== parentNode.id)
    .map((n) => {
      if (n.id === nextNode.id) {
        return {
          ...n,
          data: {
            ...n.data,
            label: operatorData.label,
            parentId: grandParentId,
            argIndex: operatorData.argIndex,
            branchType: operatorData.branchType,
          },
        };
      }
      if (grandParentId && n.id === grandParentId) {
        return replaceChildReference(n, parentNode.id, nextNode.id);
      }
      return n;
    });

  return newNodes;
}

/** True when `node` is (transitively) parented to `ancestorId`. */
function descendantOf(node: LogicNode, ancestorId: string, allNodes: LogicNode[]): boolean {
  let current = node;
  const seen = new Set<string>();
  while (current.data.parentId && !seen.has(current.id)) {
    seen.add(current.id);
    if (current.data.parentId === ancestorId) return true;
    const parent = allNodes.find((n) => n.id === current.data.parentId);
    if (!parent) return false;
    current = parent;
  }
  return false;
}

/**
 * Remove a row of a switch/match node: a Case or Then removes the whole pair
 * (both subtrees), Default removes the default row. The Match row cannot be
 * removed.
 */
function removeSwitchRow(
  nodes: LogicNode[],
  parentNode: LogicNode,
  operatorData: OperatorNodeData,
  argIndex: number
): LogicNode[] | null {
  const cell = operatorData.cells.find((c) => c.index === argIndex);
  if (!cell || cell.rowLabel === 'Match') return null;

  const partner = switchPairPartner(operatorData.cells, cell);
  const removedIndices = new Set([cell.index, ...(partner ? [partner.index] : [])]);

  const stored = storedOperandsOf(operatorData);
  const childExpr = (c: CellData): JsonLogicValue | undefined => {
    const child = c.branchId ? nodes.find((n) => n.id === c.branchId) : undefined;
    return child ? (child.data.expression ?? null) : undefined;
  };
  const values = resolveSwitchCellValues(operatorData.cells, stored, childExpr);

  // Delete the subtrees of the removed rows
  const removedRoots = [cell, ...(partner ? [partner] : [])]
    .map((c) => c.branchId)
    .filter((id): id is string => !!id);
  let newNodes = nodes.filter(
    (n) => !removedRoots.some((rootId) => n.id === rootId || descendantOf(n, rootId, nodes))
  );

  const survivors = operatorData.cells.filter((c) => !removedIndices.has(c.index));
  const args = switchArgsFromCells(survivors, values, stored.length >= 2);
  const reindexed = survivors.map((c, i) => ({ ...c, index: i }));
  const argIndexMap = new Map(survivors.map((c, i) => [c.index, i]));

  newNodes = newNodes.map((n) => {
    if (n.id === parentNode.id) {
      return {
        ...n,
        data: {
          ...operatorData,
          cells: reindexed,
          expression: { [operatorData.operator]: args },
          expressionText: undefined,
        },
      };
    }
    if (n.data.parentId === parentNode.id && n.data.argIndex !== undefined) {
      const next = argIndexMap.get(n.data.argIndex);
      if (next !== undefined && next !== n.data.argIndex) {
        return { ...n, data: { ...n.data, argIndex: next } };
      }
    }
    return n;
  });

  return newNodes;
}
