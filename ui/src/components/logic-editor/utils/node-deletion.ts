/**
 * Node Deletion Utilities
 *
 * Functions for deleting nodes while maintaining tree consistency.
 *
 * Deleting a child is parent-shape aware:
 * - n-ary operators drop the operand and reindex the remaining cells (and the
 *   surviving children's argIndex)
 * - decision diamonds keep their slots: when/then become an inline null
 *   placeholder, an else input is simply removed
 * - switch/match remove a whole Case/Then pair (deleting the partner subtree
 *   too), turn the Match into a null placeholder, or drop the Default row
 * - var drops its default; structure (template) nodes remove the element at
 *   its path and regenerate the formatted JSON
 */

import type {
  LogicNode,
  OperatorNodeData,
  StructureNodeData,
  StructureElement,
  JsonLogicValue,
  CellData,
} from '../types';
import {
  isIfOperator,
  isDecisionCells,
  decisionSlotOf,
  makeDecisionInlineCell,
} from './converters/if-else-converter';
import {
  isSwitchOperator,
  isSwitchCells,
  resolveSwitchCellValues,
  switchArgsFromCells,
  switchPairPartner,
} from './converters/switch-cells';
import {
  isVariableOperatorName,
  hasVariableCells,
  variableCellsToExpression,
  rawOperandOf,
} from './converters/variable-cells';
import {
  cloneJson,
  removeAtPath,
  shiftPathAfterRemoval,
  formatStructureWithPlaceholders,
} from './converters/structure-paths';

/**
 * Delete a node and all its descendants from the node array.
 * Also updates parent references to remove the deleted node.
 *
 * @param nodeId The ID of the node to delete
 * @param nodes The current array of nodes
 * @returns A new array with the node and its descendants removed
 */
export function deleteNodeAndDescendants(
  nodeId: string,
  nodes: LogicNode[]
): LogicNode[] {
  // Find the node being deleted to get its parent info
  const deletedNode = nodes.find((n) => n.id === nodeId);
  const parentId = deletedNode?.data.parentId;
  const parentNode = parentId ? nodes.find((n) => n.id === parentId) : undefined;

  // A switch/match case is a pair: deleting one half removes the other too.
  const rootIds = [nodeId, ...companionChildIds(parentNode, nodeId)];

  // Get all descendant IDs
  const idsToDelete = new Set<string>();
  for (const rootId of rootIds) {
    idsToDelete.add(rootId);
    for (const id of getDescendantIds(rootId, nodes)) idsToDelete.add(id);
  }

  // Filter out deleted nodes
  const filteredNodes = nodes.filter((node) => !idsToDelete.has(node.id));

  if (!parentNode) return filteredNodes;

  // Update the parent and rebuild its expression
  const { node: updatedParent, argIndexMap } = updateParentAfterChildDeletion(
    parentNode,
    new Set(rootIds),
    filteredNodes
  );

  return filteredNodes.map((node) => {
    if (node.id === parentNode.id) return updatedParent;
    // Surviving siblings keep their argIndex aligned with the reindexed cells
    if (argIndexMap && node.data.parentId === parentNode.id && node.data.argIndex !== undefined) {
      const next = argIndexMap.get(node.data.argIndex);
      if (next !== undefined && next !== node.data.argIndex) {
        return { ...node, data: { ...node.data, argIndex: next } };
      }
    }
    return node;
  });
}

/**
 * Get all descendant node IDs for a given node.
 */
export function getDescendantIds(nodeId: string, nodes: LogicNode[]): Set<string> {
  const descendants = new Set<string>();
  const node = nodes.find((n) => n.id === nodeId);

  if (!node) return descendants;

  // Get direct child IDs based on node type
  const childIds = getChildIds(node.data);

  // Recursively collect all descendants
  for (const childId of childIds) {
    descendants.add(childId);
    const childDescendants = getDescendantIds(childId, nodes);
    for (const id of childDescendants) {
      descendants.add(id);
    }
  }

  return descendants;
}

/**
 * Get direct child IDs from node data
 */
function getChildIds(data: LogicNode['data']): string[] {
  switch (data.type) {
    case 'operator': {
      const opData = data as OperatorNodeData;
      const ids: string[] = [];
      for (const cell of opData.cells) {
        if (cell.branchId) ids.push(cell.branchId);
        if (cell.conditionBranchId) ids.push(cell.conditionBranchId);
        if (cell.thenBranchId) ids.push(cell.thenBranchId);
      }
      return ids;
    }
    case 'structure': {
      const structData = data as StructureNodeData;
      return structData.elements
        .filter((el) => el.branchId)
        .map((el) => el.branchId!);
    }
    case 'literal':
    default:
      return [];
  }
}

/**
 * Children that must go together with `childId`: the other half of a
 * switch/match Case/Then pair when it is wired to its own node.
 */
function companionChildIds(parentNode: LogicNode | undefined, childId: string): string[] {
  if (!parentNode || parentNode.data.type !== 'operator') return [];
  const opData = parentNode.data as OperatorNodeData;
  if (!isSwitchOperator(opData.operator) || !isSwitchCells(opData.cells)) return [];
  const cell = opData.cells.find((c) => c.branchId === childId);
  if (!cell) return [];
  const partner = switchPairPartner(opData.cells, cell);
  return partner?.branchId ? [partner.branchId] : [];
}

/** The stored operands of an operator node's expression, normalized to an array. */
function storedOperandsOf(data: OperatorNodeData): JsonLogicValue[] {
  const raw = rawOperandOf(data.expression);
  if (raw === undefined) return [];
  return Array.isArray(raw) ? raw : [raw];
}

/** The expression of a branch cell's child (from the surviving nodes), if any. */
function childExpression(cell: CellData, allNodes: LogicNode[]): JsonLogicValue | undefined {
  if (!cell.branchId) return undefined;
  const branchNode = allNodes.find((n) => n.id === cell.branchId);
  return branchNode ? (branchNode.data.expression ?? null) : undefined;
}

interface ParentUpdate {
  node: LogicNode;
  /** old argIndex -> new argIndex for surviving children (undefined = unchanged) */
  argIndexMap?: Map<number, number>;
}

/**
 * Update a parent node after one or more of its children were deleted.
 * Creates fully immutable updates - no mutation of original objects.
 * Also rebuilds the expression from remaining children.
 */
/**
 * Rebuild a parent after some set of its children was deleted. Exported as
 * the seam `deleteNodeAndDescendants` delegates to, so callers that already
 * know the deleted set (and tests covering multi-child removal) can reuse it.
 */
export function updateParentAfterChildDeletion(
  parentNode: LogicNode,
  deletedIds: Set<string>,
  allNodes: LogicNode[]
): ParentUpdate {
  const data = parentNode.data;

  switch (data.type) {
    case 'operator':
      return updateOperatorParent(parentNode, data as OperatorNodeData, deletedIds, allNodes);
    case 'structure':
      return { node: updateStructureParent(parentNode, data as StructureNodeData, deletedIds) };
    default:
      return { node: parentNode };
  }
}

function updateOperatorParent(
  parentNode: LogicNode,
  opData: OperatorNodeData,
  deletedIds: Set<string>,
  allNodes: LogicNode[]
): ParentUpdate {
  const isDeletedCell = (cell: CellData) =>
    (cell.branchId !== undefined && deletedIds.has(cell.branchId)) ||
    (cell.conditionBranchId !== undefined && deletedIds.has(cell.conditionBranchId)) ||
    (cell.thenBranchId !== undefined && deletedIds.has(cell.thenBranchId));

  const stored = storedOperandsOf(opData);
  const withData = (cells: CellData[], expression: JsonLogicValue): LogicNode => ({
    ...parentNode,
    data: { ...opData, cells, expression, expressionText: undefined },
  });

  // Decision diamond: keep the slot layout.
  if (isIfOperator(opData.operator) && isDecisionCells(opData.cells)) {
    const cells: CellData[] = [];
    const placeholders = new Set<number>();
    for (const cell of opData.cells) {
      const slot = decisionSlotOf(cell);
      if (!isDeletedCell(cell) || !slot) {
        cells.push(cell);
      } else if (slot !== 'else') {
        // A missing when/then leaves the diamond invalid, so hold the slot with null.
        const placeholder = makeDecisionInlineCell(slot, 'null');
        placeholders.add(placeholder.index);
        cells.push(placeholder);
      }
    }
    const args: JsonLogicValue[] = cells.map((cell) =>
      placeholders.has(cell.index)
        ? null
        : (childExpression(cell, allNodes) ?? stored[cell.index] ?? null)
    );
    return { node: withData(cells, { [opData.operator]: args }) };
  }

  // Switch / match: resolve every row first (case index alignment), then drop rows.
  if (isSwitchOperator(opData.operator) && isSwitchCells(opData.cells)) {
    const values = resolveSwitchCellValues(opData.cells, stored, (cell) =>
      childExpression(cell, allNodes)
    );
    // A deleted Case or Then takes its pair partner (possibly an inline row) with it
    const droppedIndices = new Set<number>();
    for (const cell of opData.cells) {
      if (!isDeletedCell(cell) || cell.rowLabel === 'Match') continue;
      droppedIndices.add(cell.index);
      const partner = switchPairPartner(opData.cells, cell);
      if (partner) droppedIndices.add(partner.index);
    }
    const cells: CellData[] = [];
    for (const cell of opData.cells) {
      if (droppedIndices.has(cell.index)) continue;
      if (isDeletedCell(cell) && cell.rowLabel === 'Match') {
        values.set(cell.index, null);
        cells.push({ type: 'inline', icon: cell.icon, rowLabel: 'Match', label: 'null', index: cell.index });
      } else {
        cells.push(cell);
      }
    }
    const args = switchArgsFromCells(cells, values, stored.length >= 2);
    const reindexed = cells.map((cell, idx) => ({ ...cell, index: idx }));
    const argIndexMap = new Map(cells.map((cell, idx) => [cell.index, idx]));
    return { node: withData(reindexed, { [opData.operator]: args }), argIndexMap };
  }

  // Variable operators with editable cells: the default of a var is the only
  // deletable child; rebuild from the remaining cells.
  if (isVariableOperatorName(opData.operator) && hasVariableCells(opData.cells)) {
    const cells = opData.cells.filter((cell) => !isDeletedCell(cell));
    const expression =
      variableCellsToExpression(opData.operator, cells, {
        // A var without its default reads best in the plain string form
        rawOperand: opData.operator === 'var' ? undefined : rawOperandOf(opData.expression),
        resolveBranch: (cell) => childExpression(cell, allNodes),
      }) ?? { [opData.operator]: [] };
    return { node: withData(cells, expression) };
  }

  // N-ary operator: drop the operand, reindex the survivors.
  const survivors = opData.cells.filter((cell) => !isDeletedCell(cell));
  const newOperands: JsonLogicValue[] = survivors.map((cell) => {
    // Read the operand by the cell's ORIGINAL index (before reindexing)
    return childExpression(cell, allNodes) ?? stored[cell.index] ?? null;
  });
  const newCells = survivors.map((cell, idx) => ({ ...cell, index: idx }));
  const argIndexMap = new Map(survivors.map((cell, idx) => [cell.index, idx]));

  return {
    node: withData(newCells, { [opData.operator]: newOperands }),
    argIndexMap,
  };
}

function updateStructureParent(
  parentNode: LogicNode,
  structData: StructureNodeData,
  deletedIds: Set<string>
): LogicNode {
  const expression: JsonLogicValue = cloneJson(structData.expression ?? (structData.isArray ? [] : {}));
  // Survivors are tracked by their ORIGINAL slot: every removal rebuilds the
  // list into fresh objects, so identity against `structData.elements` stops
  // holding after the first one. Each removal also reads the CURRENT path,
  // which earlier removals may already have shifted down.
  let survivors: (StructureElement | null)[] = [...structData.elements];

  structData.elements.forEach((original, slot) => {
    if (!original.branchId || !deletedIds.has(original.branchId)) return;
    const removed = survivors[slot];
    if (!removed) return;
    const removedPath = removed.path ?? (removed.key !== undefined ? [removed.key] : []);
    survivors[slot] = null;
    survivors = survivors.map((el) =>
      el ? { ...el, path: shiftPathAfterRemoval(el.path, removedPath, expression) } : null
    );
    removeAtPath(expression, removedPath);
  });

  const elements = survivors.filter((el): el is StructureElement => el !== null);

  const formatted = formatStructureWithPlaceholders(expression, elements);

  return {
    ...parentNode,
    data: {
      ...structData,
      elements: formatted.elements,
      formattedJson: formatted.formattedJson,
      expression,
      expressionText: undefined,
    },
  };
}

/**
 * Check if a node is the root node (has no parent)
 */
export function isRootNode(node: LogicNode): boolean {
  return !node.data.parentId;
}

/**
 * Check if a node can be deleted.
 * Root nodes cannot be deleted.
 */
export function canDeleteNode(node: LogicNode): boolean {
  return !isRootNode(node);
}
