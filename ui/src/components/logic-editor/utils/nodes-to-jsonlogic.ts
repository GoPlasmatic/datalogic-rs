/**
 * Nodes to JSONLogic Serializer
 *
 * Converts a tree of visual nodes back to a JSONLogic expression.
 *
 * The serializer is the inverse of jsonLogicToNodes: for every builtin
 * operator and shape (if / else-if chains, switch cases, val scope paths, var
 * defaults, templating structures, single-value shorthand) the round trip
 * jsonLogicToNodes -> nodesToJsonLogic returns the input unchanged. Cells are
 * the source of truth for anything the editor can change; the node's stored
 * expression supplies inline values and the original argument form.
 */

import type {
  LogicNode,
  LogicNodeData,
  LiteralNodeData,
  OperatorNodeData,
  StructureNodeData,
  CellData,
} from '../types';
import type { JsonLogicValue } from '../types/jsonlogic';
import {
  isIfOperator,
  isDecisionCells,
  decisionCell,
} from './converters/if-else-converter';
import {
  isSwitchOperator,
  isSwitchCells,
  resolveSwitchCellValues,
  switchArgsFromCells,
} from './converters/switch-cells';
import {
  isVariableOperatorName,
  hasVariableCells,
  variableCellsToExpression,
  rawOperandOf,
} from './converters/variable-cells';
import { cloneJson, setAtPath } from './converters/structure-paths';

/**
 * Convert a tree of visual nodes back to JSONLogic
 * @param nodes The array of all nodes
 * @returns The JSONLogic expression, or null if no root node
 */
export function nodesToJsonLogic(nodes: LogicNode[]): JsonLogicValue | null {
  if (nodes.length === 0) {
    return null;
  }

  // Find the root node (node with no parentId)
  const rootNode = nodes.find((n) => !n.data.parentId);
  if (!rootNode) {
    // Fallback: use first node if no explicit root
    return nodeToJsonLogic(nodes[0], nodes);
  }

  return nodeToJsonLogic(rootNode, nodes);
}

/**
 * Convert a single node to JSONLogic, recursively processing children
 */
function nodeToJsonLogic(node: LogicNode, allNodes: LogicNode[]): JsonLogicValue {
  const nodeMap = new Map(allNodes.map((n) => [n.id, n]));

  return convertNode(node.data, nodeMap);
}

/**
 * Convert node data to JSONLogic expression
 */
function convertNode(
  data: LogicNodeData,
  nodeMap: Map<string, LogicNode>
): JsonLogicValue {
  switch (data.type) {
    case 'literal':
      return convertLiteral(data);
    case 'operator':
      return convertOperator(data, nodeMap);
    case 'structure':
      return convertStructure(data, nodeMap);
    default:
      // Fallback: return the stored expression if available
      return (data as LogicNodeData).expression ?? null;
  }
}

/**
 * Convert literal node to JSONLogic
 */
function convertLiteral(data: LiteralNodeData): JsonLogicValue {
  return data.value;
}

/** The operands of a node's stored expression, normalized to an array. */
function storedOperandsOf(data: OperatorNodeData): JsonLogicValue[] {
  const raw = rawOperandOf(data.expression);
  if (raw === undefined) return [];
  return Array.isArray(raw) ? raw : [raw];
}

/** Resolve a cell: branch -> its child's expression, otherwise the stored operand. */
function resolveCellValue(
  cell: CellData,
  storedValue: JsonLogicValue,
  nodeMap: Map<string, LogicNode>
): JsonLogicValue {
  const resolved = resolveBranchCell(cell, nodeMap);
  return resolved === undefined ? storedValue : resolved;
}

/** The converted child of a branch cell, or undefined when the cell is not a (live) branch. */
function resolveBranchCell(
  cell: CellData,
  nodeMap: Map<string, LogicNode>
): JsonLogicValue | undefined {
  if (cell.type === 'branch' && cell.branchId) {
    const branchNode = nodeMap.get(cell.branchId);
    if (branchNode) {
      return convertNode(branchNode.data, nodeMap);
    }
  }
  return undefined;
}

/**
 * Convert operator node to JSONLogic
 *
 * This unified function handles all operator types including:
 * - Decision diamonds (if / ?:): reconstruct the flat if/else-if chain
 * - Switch / match: reconstruct the nested [[case, result], ...] form
 * - Variable operators (var, val, exists): reconstruct from editable cells
 * - Everything else: cells in index order (inline values from the stored expression)
 */
function convertOperator(
  data: OperatorNodeData,
  nodeMap: Map<string, LogicNode>
): JsonLogicValue {
  const rawOperand = rawOperandOf(data.expression);
  const storedOperands = storedOperandsOf(data);

  if (isIfOperator(data.operator) && isDecisionCells(data.cells)) {
    return { [data.operator]: convertDecisionArgs(data, nodeMap) };
  }

  if (isSwitchOperator(data.operator) && isSwitchCells(data.cells)) {
    return convertSwitchFromCells(data, storedOperands, nodeMap);
  }

  if (isVariableOperatorName(data.operator) && hasVariableCells(data.cells)) {
    const rebuilt = variableCellsToExpression(data.operator, data.cells, {
      rawOperand,
      resolveBranch: (cell) => resolveBranchCell(cell, nodeMap),
    });
    if (rebuilt !== undefined) return rebuilt;
    // A foreign cell layout we cannot rebuild from: the stored expression is
    // the only faithful source.
    if (data.expression !== undefined) return data.expression;
  }

  // Build a map of cell index -> cell for quick lookup
  const cellByIndex = new Map<number, CellData>();
  for (const cell of data.cells) {
    cellByIndex.set(cell.index, cell);
  }

  // Build the result array by processing cells in index order. Indices past
  // the last cell are padded from the stored expression (a node without cells
  // keeps its operands there).
  const resultArgs: JsonLogicValue[] = [];
  const maxIndex = Math.max(
    storedOperands.length - 1,
    ...data.cells.map((c) => c.index)
  );

  for (let i = 0; i <= maxIndex; i++) {
    const cell = cellByIndex.get(i);

    if (cell) {
      if (cell.type === 'branch' && cell.branchId) {
        // Branch cell - use child node's expression
        const branchNode = nodeMap.get(cell.branchId);
        if (branchNode) {
          resultArgs.push(convertNode(branchNode.data, nodeMap));
        } else if (i < storedOperands.length) {
          resultArgs.push(storedOperands[i]);
        }
      } else if (cell.conditionBranchId || cell.thenBranchId) {
        // If/then cell - has separate condition and then branches
        if (cell.conditionBranchId) {
          const condNode = nodeMap.get(cell.conditionBranchId);
          if (condNode) {
            resultArgs.push(convertNode(condNode.data, nodeMap));
          }
        }
        if (cell.thenBranchId) {
          const thenNode = nodeMap.get(cell.thenBranchId);
          if (thenNode) {
            resultArgs.push(convertNode(thenNode.data, nodeMap));
          }
        }
      } else if (i < storedOperands.length) {
        // Inline or editable cell - use value from stored expression
        resultArgs.push(storedOperands[i]);
      }
    } else if (i < storedOperands.length) {
      // No cell for this index - use stored expression value
      resultArgs.push(storedOperands[i]);
    }
  }

  // Single-value shorthand ({"!": true}, {"cat": "a"}, {"length": {"var": "x"}})
  // is emitted back in the form it was written in.
  if (
    rawOperand !== undefined &&
    !Array.isArray(rawOperand) &&
    resultArgs.length === 1 &&
    data.cells.every((c) => c.index === 0)
  ) {
    return { [data.operator]: resultArgs[0] };
  }

  return { [data.operator]: resultArgs };
}

/**
 * Rebuild the flat [cond, then, cond, then, ..., else] argument list of a
 * decision diamond. An else input wired to an else-if diamond (label 'elif')
 * is spliced back into the list instead of being nested.
 */
function convertDecisionArgs(
  data: OperatorNodeData,
  nodeMap: Map<string, LogicNode>
): JsonLogicValue[] {
  const stored = storedOperandsOf(data);
  const whenCell = decisionCell(data.cells, 'when');
  const thenCell = decisionCell(data.cells, 'then');
  const elseCell = decisionCell(data.cells, 'else');

  const args: JsonLogicValue[] = [
    whenCell ? resolveCellValue(whenCell, stored[whenCell.index] ?? null, nodeMap) : null,
    thenCell ? resolveCellValue(thenCell, stored[thenCell.index] ?? null, nodeMap) : null,
  ];

  if (!elseCell) return args;

  const elseNode = elseCell.type === 'branch' && elseCell.branchId ? nodeMap.get(elseCell.branchId) : undefined;
  if (
    elseNode &&
    elseNode.data.type === 'operator' &&
    elseNode.data.label === 'elif' &&
    isIfOperator(elseNode.data.operator) &&
    isDecisionCells(elseNode.data.cells)
  ) {
    args.push(...convertDecisionArgs(elseNode.data, nodeMap));
    return args;
  }

  args.push(resolveCellValue(elseCell, stored[elseCell.index] ?? null, nodeMap));
  return args;
}

/**
 * Convert switch/match operator from cells
 * Reconstructs {"switch": [discriminant, [[case1, result1], ...], default]}
 *
 * storedOperands = [discriminant, [[case1, result1], [case2, result2], ...], default]
 * cells = [Match(0), Case(1), Then(2), Case(3), Then(4), ..., Default(N)]
 */
function convertSwitchFromCells(
  data: OperatorNodeData,
  storedOperands: JsonLogicValue[],
  nodeMap: Map<string, LogicNode>
): JsonLogicValue {
  const values = resolveSwitchCellValues(data.cells, storedOperands, (cell) =>
    resolveBranchCell(cell, nodeMap)
  );
  const args = switchArgsFromCells(data.cells, values, storedOperands.length >= 2);
  return { [data.operator]: args };
}

/**
 * Convert structure node to JSONLogic (object/array with embedded expressions).
 *
 * The node keeps its complete JSON value on `data.expression`; each recorded
 * element is substituted at its path with the converted child, so literal
 * fields, nesting and array literals all survive.
 */
function convertStructure(
  data: StructureNodeData,
  nodeMap: Map<string, LogicNode>
): JsonLogicValue {
  let result: JsonLogicValue =
    data.expression !== undefined && data.expression !== null
      ? cloneJson(data.expression)
      : data.isArray ? [] : {};

  for (const element of data.elements) {
    const path = element.path ?? (element.key !== undefined ? [element.key] : undefined);
    if (!path) continue;
    if (element.type === 'inline') {
      result = setAtPath(result, path, element.value ?? null);
    } else if (element.branchId) {
      const branchNode = nodeMap.get(element.branchId);
      if (branchNode) {
        result = setAtPath(result, path, convertNode(branchNode.data, nodeMap));
      }
    }
  }
  return result;
}

/**
 * Get the root node from an array of nodes
 */
export function getRootNode(nodes: LogicNode[]): LogicNode | null {
  if (nodes.length === 0) return null;
  return nodes.find((n) => !n.data.parentId) ?? nodes[0];
}
