/**
 * Nodes to JSONLogic Serializer
 *
 * Converts a tree of visual nodes back to a JSONLogic expression.
 */

import type {
  LogicNode,
  LogicNodeData,
  LiteralNodeData,
  OperatorNodeData,
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

/**
 * Convert operator node to JSONLogic
 *
 * This unified function handles all operator types including:
 * - Variable operators (var, val, exists): reconstruct from cells' editable values
 * - Standard operators: reconstruct from cells (both inline and branch)
 * - Decision operators (if): reconstruct from cells with condition/then branches
 */
function convertOperator(
  data: OperatorNodeData,
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
  const resultArgs: JsonLogicValue[] = [];
  const maxIndex = Math.max(
    storedOperands.length - 1,
    ...data.cells.map((c) => c.index)
  );

  for (let i = 0; i <= maxIndex; i++) {
    const cell = cellByIndex.get(i);

    if (cell) {
      if (cell.type === 'editable') {
        // Editable cell - use value from stored expression
        if (i < storedOperands.length) {
          resultArgs.push(storedOperands[i]);
        }
      } else if (cell.type === 'branch' && cell.branchId) {
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

  // Special handling for val: reconstruct from editable cell values
  // Val format: {"val": [[-N], "comp1", "comp2"]} or {"val": "metadata"}
  if (data.operator === 'val') {
    return convertValFromCells(data);
  }

  // For var and exists with a single argument, unwrap from array
  // e.g., {"var": "path"} instead of {"var": ["path"]}
  // and {"exists": "item"} instead of {"exists": ["item"]}
  if ((data.operator === 'var' || data.operator === 'exists') && resultArgs.length === 1) {
    return { [data.operator]: resultArgs[0] };
  }

  // Special handling for var: reconstruct from editable cell values
  if (data.operator === 'var') {
    return convertVarFromCells(data, resultArgs, nodeMap);
  }

  return { [data.operator]: resultArgs };
}

/**
 * Convert val operator from editable cell values
 * Reconstructs {"val": [[-N], "comp1", "comp2"]} or {"val": "metadata"}
 */
function convertValFromCells(data: OperatorNodeData): JsonLogicValue {
  const scopeCell = data.cells.find((c) => c.fieldId === 'scopeLevel');
  const pathCells = data.cells.filter((c) => c.fieldId === 'path');

  const scopeJump = typeof scopeCell?.value === 'number' ? scopeCell.value : 0;
  const pathComponents: string[] = [];

  for (const pc of pathCells) {
    const pathStr = String(pc.value ?? '');
    if (pathStr) {
      // Split dot-separated path into components
      pathStr.split('.').forEach((comp) => {
        if (comp) pathComponents.push(comp);
      });
    }
  }

  // Simple metadata access: {"val": "index"} or {"val": "key"}
  if (scopeJump === 0 && pathComponents.length === 1 &&
      (pathComponents[0] === 'index' || pathComponents[0] === 'key')) {
    return { val: pathComponents[0] };
  }

  // Build array: [[-N], "comp1", "comp2", ...]
  const args: JsonLogicValue[] = [];
  if (scopeJump > 0) {
    args.push([-scopeJump]);
  }
  args.push(...pathComponents);

  // If no scope and no path, return empty array
  if (args.length === 0) {
    return { val: [] };
  }

  return { val: args };
}

/**
 * Convert var operator from editable cell values
 * Reconstructs {"var": "path"} or {"var": ["path", default]}
 */
function convertVarFromCells(
  data: OperatorNodeData,
  resultArgs: JsonLogicValue[],
  nodeMap: Map<string, LogicNode>
): JsonLogicValue {
  const pathCell = data.cells.find((c) => c.fieldId === 'path');
  const pathValue = pathCell ? String(pathCell.value ?? '') : '';

  // Check if there's a default value cell (index 1, not a path field)
  const defaultCell = data.cells.find((c) => c.index === 1 && c.fieldId !== 'path');
  if (defaultCell) {
    let defaultValue: JsonLogicValue;
    if (defaultCell.type === 'branch' && defaultCell.branchId) {
      const branchNode = nodeMap.get(defaultCell.branchId);
      defaultValue = branchNode ? convertNode(branchNode.data, nodeMap) : null;
    } else if (defaultCell.type === 'inline') {
      // Use stored operands for inline default
      defaultValue = resultArgs.length > 1 ? resultArgs[1] : null;
    } else {
      defaultValue = resultArgs.length > 1 ? resultArgs[1] : null;
    }
    return { var: [pathValue, defaultValue] };
  }

  return { var: pathValue };
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
