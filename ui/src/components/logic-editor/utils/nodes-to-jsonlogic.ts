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
 */
function convertOperator(
  data: OperatorNodeData,
  nodeMap: Map<string, LogicNode>
): JsonLogicValue {
  const children = data.childIds
    .map((id) => nodeMap.get(id))
    .filter((node): node is LogicNode => node !== undefined)
    .sort((a, b) => (a.data.argIndex ?? 0) - (b.data.argIndex ?? 0))
    .map((node) => convertNode(node.data, nodeMap));

  // Handle special cases for operators
  if (children.length === 0) {
    // Operators with no arguments
    return { [data.operator]: [] };
  }

  if (children.length === 1) {
    // Single argument - check if we need array wrapper
    // Most operators use array format even for single args
    return { [data.operator]: children };
  }

  return { [data.operator]: children };
}

/**
 * Convert vertical cell node to JSONLogic
 * Handles multi-argument operators like if/then/else, comparison chains, etc.
 */
function convertVerticalCell(
  data: VerticalCellNodeData,
  nodeMap: Map<string, LogicNode>
): JsonLogicValue {
  const args: JsonLogicValue[] = [];

  // Process cells in order
  for (const cell of data.cells) {
    if (cell.type === 'branch' && cell.branchId) {
      const branchNode = nodeMap.get(cell.branchId);
      if (branchNode) {
        args.push(convertNode(branchNode.data, nodeMap));
      }
    } else if (cell.conditionBranchId || cell.thenBranchId) {
      // If/then cell - has separate condition and then branches
      if (cell.conditionBranchId) {
        const condNode = nodeMap.get(cell.conditionBranchId);
        if (condNode) {
          args.push(convertNode(condNode.data, nodeMap));
        }
      }
      if (cell.thenBranchId) {
        const thenNode = nodeMap.get(cell.thenBranchId);
        if (thenNode) {
          args.push(convertNode(thenNode.data, nodeMap));
        }
      }
    }
  }

  return { [data.operator]: args };
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
