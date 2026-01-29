/**
 * Nodes to JSONLogic Serializer
 *
 * Converts a tree of visual nodes back to a JSONLogic expression.
 */

import type {
  LogicNode,
  LogicNodeData,
  LiteralNodeData,
  VariableNodeData,
  OperatorNodeData,
  VerticalCellNodeData,
  DecisionNodeData,
  StructureNodeData,
} from '../types';
import type { JsonLogicValue } from '../types/jsonlogic';

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
    case 'variable':
      return convertVariable(data);
    case 'operator':
      return convertOperator(data, nodeMap);
    case 'verticalCell':
      return convertVerticalCell(data, nodeMap);
    case 'decision':
      return convertDecision(data, nodeMap);
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

/**
 * Convert variable node to JSONLogic
 */
function convertVariable(data: VariableNodeData): JsonLogicValue {
  // Return the stored expression - it's already in JSONLogic format
  // The expression is updated when panel values change
  return data.expression ?? { [data.operator]: data.path };
}

/**
 * Convert operator node to JSONLogic
 *
 * This function handles the case where operands can be:
 * 1. Child nodes (stored in childIds with argIndex)
 * 2. Inline literals (stored directly in the expression, no child node)
 *
 * We merge both sources: use child node expressions where available,
 * fall back to inline literals from the stored expression.
 */
function convertOperator(
  data: OperatorNodeData,
  nodeMap: Map<string, LogicNode>
): JsonLogicValue {
  // Build a map of argIndex -> child node for quick lookup
  const childByArgIndex = new Map<number, LogicNode>();
  for (const id of data.childIds) {
    const node = nodeMap.get(id);
    if (node && node.data.argIndex !== undefined) {
      childByArgIndex.set(node.data.argIndex, node);
    }
  }

  // Get the operands from the stored expression to preserve inline literals
  let storedOperands: JsonLogicValue[] = [];
  if (data.expression && typeof data.expression === 'object' && !Array.isArray(data.expression)) {
    const opKey = Object.keys(data.expression)[0];
    const operands = (data.expression as Record<string, unknown>)[opKey];
    storedOperands = Array.isArray(operands) ? operands : [operands as JsonLogicValue];
  }

  // Build the result array by merging child nodes with stored operands
  const resultOperands: JsonLogicValue[] = [];
  const maxIndex = Math.max(storedOperands.length - 1, ...Array.from(childByArgIndex.keys()));

  for (let i = 0; i <= maxIndex; i++) {
    const childNode = childByArgIndex.get(i);
    if (childNode) {
      // Use the child node's expression (authoritative for complex expressions)
      resultOperands.push(convertNode(childNode.data, nodeMap));
    } else if (i < storedOperands.length) {
      // Use the inline literal from the stored expression
      resultOperands.push(storedOperands[i]);
    }
  }

  // Handle edge case: no operands
  if (resultOperands.length === 0) {
    return { [data.operator]: [] };
  }

  return { [data.operator]: resultOperands };
}

/**
 * Convert vertical cell node to JSONLogic
 * Handles multi-argument operators like if/then/else, comparison chains, etc.
 *
 * This function handles the case where operands can be:
 * 1. Branch cells (with branchId pointing to a child node)
 * 2. Inline cells (simple literals stored in the expression, no child node)
 *
 * We use the stored expression as the source of truth for inline cells,
 * and child node expressions for branch cells.
 */
function convertVerticalCell(
  data: VerticalCellNodeData,
  nodeMap: Map<string, LogicNode>
): JsonLogicValue {
  // Get the operands from the stored expression to preserve inline literals
  let storedOperands: JsonLogicValue[] = [];
  if (data.expression && typeof data.expression === 'object' && !Array.isArray(data.expression)) {
    const opKey = Object.keys(data.expression)[0];
    const operands = (data.expression as Record<string, unknown>)[opKey];
    storedOperands = Array.isArray(operands) ? operands : [operands as JsonLogicValue];
  }

  // Build a map of cell index -> cell for quick lookup
  const cellByIndex = new Map<number, typeof data.cells[0]>();
  for (const cell of data.cells) {
    cellByIndex.set(cell.index, cell);
  }

  // Build the result array by processing cells in index order
  // Use stored expression for inline cells, child nodes for branch cells
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
          // Fallback to stored expression if node not found
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
      } else if (cell.type === 'inline') {
        // Inline cell - use value from stored expression
        if (i < storedOperands.length) {
          resultArgs.push(storedOperands[i]);
        }
      }
    } else if (i < storedOperands.length) {
      // No cell for this index - use stored expression value
      resultArgs.push(storedOperands[i]);
    }
  }

  return { [data.operator]: resultArgs };
}

/**
 * Convert decision node to JSONLogic (if/then/else tree)
 */
function convertDecision(
  data: DecisionNodeData,
  nodeMap: Map<string, LogicNode>
): JsonLogicValue {
  const args: JsonLogicValue[] = [];

  // Condition
  if (data.isConditionComplex && data.conditionBranchId) {
    const condNode = nodeMap.get(data.conditionBranchId);
    if (condNode) {
      args.push(convertNode(condNode.data, nodeMap));
    }
  } else {
    // Simple condition is stored in conditionExpression
    args.push(data.conditionExpression);
  }

  // Yes branch (then)
  const yesNode = nodeMap.get(data.yesBranchId);
  if (yesNode) {
    args.push(convertNode(yesNode.data, nodeMap));
  }

  // No branch (else) - could be another decision node for else-if chains
  const noNode = nodeMap.get(data.noBranchId);
  if (noNode) {
    args.push(convertNode(noNode.data, nodeMap));
  }

  return { if: args };
}

/**
 * Convert structure node to JSONLogic (object/array with embedded expressions)
 */
function convertStructure(
  data: StructureNodeData,
  nodeMap: Map<string, LogicNode>
): JsonLogicValue {
  if (data.isArray) {
    // Array structure
    const elements: JsonLogicValue[] = [];
    for (const element of data.elements) {
      if (element.type === 'inline') {
        elements.push(element.value ?? null);
      } else if (element.branchId) {
        const branchNode = nodeMap.get(element.branchId);
        if (branchNode) {
          elements.push(convertNode(branchNode.data, nodeMap));
        }
      }
    }
    return elements;
  }

  // Object structure
  const obj: Record<string, JsonLogicValue> = {};
  for (const element of data.elements) {
    if (element.key) {
      if (element.type === 'inline') {
        obj[element.key] = element.value ?? null;
      } else if (element.branchId) {
        const branchNode = nodeMap.get(element.branchId);
        if (branchNode) {
          obj[element.key] = convertNode(branchNode.data, nodeMap);
        }
      }
    }
  }
  return obj;
}

/**
 * Get the root node from an array of nodes
 */
export function getRootNode(nodes: LogicNode[]): LogicNode | null {
  if (nodes.length === 0) return null;
  return nodes.find((n) => !n.data.parentId) ?? nodes[0];
}
