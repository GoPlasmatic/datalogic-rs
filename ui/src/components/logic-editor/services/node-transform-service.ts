/**
 * Node Transform Service
 *
 * Provides pure functions for transforming nodes (wrapping, duplicating).
 */

import { v4 as uuidv4 } from 'uuid';
import type {
  LogicNode,
  OperatorNodeData,
  StructureNodeData,
  LiteralNodeData,
  JsonLogicValue,
  CellData,
} from '../types';
import { getOperator } from '../config/operators';
import {
  cloneNodesWithIdMapping,
  getDescendants,
  updateParentChildReference,
  replaceChildReference,
} from '../utils/node-cloning';
import {
  isIfOperator,
  isDecisionCells,
  makeDecisionBranchCell,
} from '../utils/converters/if-else-converter';
import {
  isSwitchOperator,
  isSwitchCells,
  resolveSwitchCellValues,
  switchArgsFromCells,
  switchPairPartner,
} from '../utils/converters/switch-cells';
import { hasVariableCells, rawOperandOf } from '../utils/converters/variable-cells';
import { cloneJson, setAtPath, formatStructureWithPlaceholders } from '../utils/converters/structure-paths';
import { canEditArguments } from './argument-service';

/** The stored operands of a node's expression, normalized to an array. */
function storedOperandsOf(data: OperatorNodeData): JsonLogicValue[] {
  const raw = rawOperandOf(data.expression);
  if (raw === undefined) return [];
  return Array.isArray(raw) ? raw : [raw];
}

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
 * Wrap a node in an operator: the node becomes the wrapper's first operand
 * and the wrapper takes the node's place under its parent (operator cells and
 * structure elements alike). Wrapping in `if` builds a real decision diamond
 * with the node as its condition and placeholder then/else values.
 */
export function wrapInOperator(
  nodes: LogicNode[],
  nodeId: string,
  operator: string
): LogicNode[] | null {
  const targetNode = nodes.find((n) => n.id === nodeId);
  if (!targetNode) return null;

  const newOperatorId = uuidv4();
  const opConfig = getOperator(operator);
  const targetExpression = targetNode.data.expression ?? null;
  const extraNodes: LogicNode[] = [];

  let cells: CellData[];
  let expression: JsonLogicValue;
  let label = opConfig?.label || operator;
  let icon: OperatorNodeData['icon'] = 'list';
  let targetBranchType: LiteralNodeData['branchType'] | undefined;

  if (isIfOperator(operator)) {
    const thenNode = literalNode('yes', 'string', newOperatorId, 1, 'yes');
    const elseNode = literalNode('no', 'string', newOperatorId, 2, 'no');
    extraNodes.push(thenNode, elseNode);
    cells = [
      makeDecisionBranchCell('when', nodeId),
      makeDecisionBranchCell('then', thenNode.id, '"yes"'),
      makeDecisionBranchCell('else', elseNode.id, '"no"'),
    ];
    expression = { [operator]: [targetExpression, 'yes', 'no'] };
    label = 'if';
    icon = 'diamond';
    targetBranchType = 'condition';
  } else {
    cells = [{ type: 'branch', branchId: nodeId, index: 0 }];
    expression = { [operator]: [targetExpression] };
  }

  const wrapperNode: LogicNode = {
    id: newOperatorId,
    type: 'operator',
    position: { x: 0, y: 0 },
    data: {
      type: 'operator',
      operator,
      category: opConfig?.category || 'logical',
      label,
      icon,
      cells,
      expression,
      parentId: targetNode.data.parentId,
      argIndex: targetNode.data.argIndex,
      branchType: targetNode.data.branchType,
    } as OperatorNodeData,
  };

  const updatedTarget: LogicNode = {
    ...targetNode,
    data: {
      ...targetNode.data,
      parentId: newOperatorId,
      argIndex: 0,
      branchType: targetBranchType,
    },
  };

  const updatedNodes = nodes.map((n) => {
    if (n.id === nodeId) {
      return updatedTarget;
    }
    // Update the parent's reference (cells or structure elements) to the wrapper
    if (n.id === targetNode.data.parentId) {
      return replaceChildReference(n, nodeId, newOperatorId);
    }
    return n;
  });

  return [...updatedNodes, wrapperNode, ...extraNodes];
}

/**
 * Duplicate a node and its descendants as a new sibling.
 *
 * Only parents with a free slot accept the copy: n-ary operators append an
 * operand, switch/match duplicate the whole Case/Then pair, array templates
 * append an element. Fixed-slot parents (decision diamonds, var defaults,
 * switch Match/Default rows, object templates) return null so the caller
 * leaves the graph unchanged. A root node is duplicated in place (the tree is
 * replaced by its copy).
 */
export function duplicateNodeTree(
  nodes: LogicNode[],
  nodeId: string
): { nodes: LogicNode[]; newRootId: string } | null {
  const targetNode = nodes.find((n) => n.id === nodeId);
  if (!targetNode) return null;

  const descendants = getDescendants(nodeId, nodes);
  const nodesToClone = [targetNode, ...descendants];

  const { nodes: clonedNodes, newRootId } = cloneNodesWithIdMapping(nodesToClone, nodeId);
  const clonedRoot = clonedNodes.find((n) => n.id === newRootId)!;

  // If the original had a parent, add as sibling
  if (targetNode.data.parentId) {
    const parent = nodes.find((n) => n.id === targetNode.data.parentId);
    if (!parent) return null;

    if (parent.data.type === 'operator') {
      return duplicateUnderOperator(nodes, parent, targetNode, clonedNodes, clonedRoot);
    }
    if (parent.data.type === 'structure') {
      return duplicateUnderStructure(nodes, parent, clonedNodes, clonedRoot);
    }
    return null;
  }

  // No parent: replace the entire tree with its copy
  clonedRoot.data = {
    ...clonedRoot.data,
    parentId: undefined,
    argIndex: undefined,
  };

  return { nodes: clonedNodes, newRootId };
}

function duplicateUnderOperator(
  nodes: LogicNode[],
  parent: LogicNode,
  targetNode: LogicNode,
  clonedNodes: LogicNode[],
  clonedRoot: LogicNode
): { nodes: LogicNode[]; newRootId: string } | null {
  const opData = parent.data as OperatorNodeData;
  const stored = storedOperandsOf(opData);

  // Fixed-slot parents: a diamond input or a variable default has no free slot
  if (isIfOperator(opData.operator) && isDecisionCells(opData.cells)) return null;
  if (hasVariableCells(opData.cells)) return null;

  // Switch / match: duplicate the whole Case/Then pair
  if (isSwitchOperator(opData.operator) && isSwitchCells(opData.cells)) {
    const cell = opData.cells.find((c) => c.branchId === targetNode.id);
    if (!cell || (cell.rowLabel !== 'Case' && cell.rowLabel !== 'Then')) return null;
    const partner = switchPairPartner(opData.cells, cell);
    if (!partner) return null;

    const values = resolveSwitchCellValues(opData.cells, stored, (c) => {
      const child = c.branchId ? nodes.find((n) => n.id === c.branchId) : undefined;
      return child ? (child.data.expression ?? null) : undefined;
    });

    const defaultPos = opData.cells.findIndex((c) => c.rowLabel === 'Default');
    const insertAt = defaultPos === -1 ? opData.cells.length : defaultPos;
    const pairCells = cell.rowLabel === 'Case' ? [cell, partner] : [partner, cell];

    // Clone the partner subtree too, when it is wired
    const extraClones: LogicNode[] = [];
    let partnerCloneId: string | undefined;
    if (partner.branchId) {
      const partnerNode = nodes.find((n) => n.id === partner.branchId);
      if (partnerNode) {
        const partnerClone = cloneNodesWithIdMapping(
          [partnerNode, ...getDescendants(partnerNode.id, nodes)],
          partnerNode.id
        );
        extraClones.push(...partnerClone.nodes);
        partnerCloneId = partnerClone.newRootId;
      }
    }

    const newCells: CellData[] = pairCells.map((c, i) => {
      const cloneId = c === cell ? clonedRoot.id : partnerCloneId;
      return {
        ...c,
        index: insertAt + i,
        branchId: c.type === 'branch' ? cloneId : undefined,
      };
    });
    const shifted = new Map<number, JsonLogicValue>();
    for (const [index, value] of values) {
      shifted.set(index >= insertAt ? index + 2 : index, value);
    }
    pairCells.forEach((c, i) => shifted.set(insertAt + i, values.get(c.index) ?? null));

    const cells = [...opData.cells];
    cells.splice(insertAt, 0, ...newCells);
    const reindexed = cells.map((c, idx) => ({ ...c, index: idx }));
    const args = switchArgsFromCells(reindexed, shifted, true);

    const allClones = [...clonedNodes, ...extraClones];
    for (const clone of allClones) {
      if (clone.id === clonedRoot.id) {
        clone.data = { ...clone.data, argIndex: newCells.find((c) => c.branchId === clonedRoot.id)?.index };
      } else if (clone.id === partnerCloneId) {
        clone.data = { ...clone.data, parentId: parent.id, argIndex: newCells.find((c) => c.branchId === partnerCloneId)?.index };
      }
    }

    const updatedNodes = nodes.map((n) => {
      if (n.id === parent.id) {
        return {
          ...n,
          data: { ...opData, cells: reindexed, expression: { [opData.operator]: args }, expressionText: undefined },
        };
      }
      if (n.data.parentId === parent.id && (n.data.argIndex ?? 0) >= insertAt) {
        return { ...n, data: { ...n.data, argIndex: (n.data.argIndex ?? 0) + 2 } };
      }
      return n;
    });
    return { nodes: [...updatedNodes, ...allClones], newRootId: clonedRoot.id };
  }

  // N-ary operator: append the copy as a new operand
  if (!canEditArguments(opData.operator)) return null;
  const max = getOperator(opData.operator)?.arity.max;
  if (max !== undefined && opData.cells.length >= max) return null;

  const newArgIndex = opData.cells.length;
  clonedRoot.data = { ...clonedRoot.data, argIndex: newArgIndex };

  const updatedNodes = nodes.map((n) => {
    if (n.id === parent.id) {
      return {
        ...n,
        data: {
          ...opData,
          cells: [...opData.cells, { type: 'branch' as const, branchId: clonedRoot.id, index: newArgIndex }],
          expression: { [opData.operator]: [...stored, clonedRoot.data.expression ?? null] },
          expressionText: undefined,
        },
      };
    }
    return n;
  });

  return { nodes: [...updatedNodes, ...clonedNodes], newRootId: clonedRoot.id };
}

function duplicateUnderStructure(
  nodes: LogicNode[],
  parent: LogicNode,
  clonedNodes: LogicNode[],
  clonedRoot: LogicNode
): { nodes: LogicNode[]; newRootId: string } | null {
  const structData = parent.data as StructureNodeData;
  // An object template has no free slot (its keys are fixed); an array does.
  if (!structData.isArray) return null;

  const expression: JsonLogicValue = cloneJson(structData.expression ?? []);
  const base = Array.isArray(expression) ? expression : [];
  const newIndex = base.length;
  const path = [String(newIndex)];
  const withElement = setAtPath(base, path, clonedRoot.data.expression ?? null);

  const elements = [
    ...structData.elements,
    {
      type: 'expression' as const,
      path,
      branchId: clonedRoot.id,
      startOffset: 0,
      endOffset: 0,
    },
  ];
  const formatted = formatStructureWithPlaceholders(withElement, elements);
  const expressionIndex = elements.filter((el) => el.type === 'expression').length - 1;

  clonedRoot.data = { ...clonedRoot.data, argIndex: expressionIndex };

  const updatedNodes = nodes.map((n) => {
    if (n.id === parent.id) {
      return {
        ...n,
        data: {
          ...structData,
          elements: formatted.elements,
          formattedJson: formatted.formattedJson,
          expression: withElement,
          expressionText: undefined,
        },
      };
    }
    return n;
  });

  return { nodes: [...updatedNodes, ...clonedNodes], newRootId: clonedRoot.id };
}

// Re-export cloning utilities used elsewhere
export { cloneNodesWithIdMapping, getDescendants, updateParentChildReference, replaceChildReference };
