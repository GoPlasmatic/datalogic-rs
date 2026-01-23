import { v4 as uuidv4 } from 'uuid';
import type { JsonLogicValue, StructureNodeData, StructureElement, LogicNode } from '../../types';
import type { ConversionContext, ConverterFn } from './types';
import { getParentInfo } from './types';
import { createArgEdge, createBranchEdge } from '../node-factory';
import { isJsonLogicExpression } from '../type-helpers';
import { generateExpressionText } from '../formatting';

// Placeholder marker used in formatted JSON for expressions
const EXPR_PLACEHOLDER = '{{EXPR}}';
// The placeholder as it appears in JSON.stringify output (with quotes)
const EXPR_PLACEHOLDER_QUOTED = `"${EXPR_PLACEHOLDER}"`;

/**
 * Convert a data structure (object or array with potential JSONLogic expressions)
 * to a structure node that displays formatted JSON with linked expression branches.
 */
export function convertStructure(
  value: Record<string, unknown> | unknown[],
  context: ConversionContext,
  convertValue: ConverterFn
): string {
  const parentInfo = getParentInfo(context);
  const nodeId = uuidv4();
  const isArray = Array.isArray(value);

  // Collect elements and build formatted JSON with placeholders
  const elements: StructureElement[] = [];
  let expressionIndex = 0;

  // Build a modified structure for JSON formatting with placeholders
  const structureWithPlaceholders = walkAndCollect(
    value,
    [],
    (path, item, key) => {
      if (isJsonLogicExpression(item)) {
        // This is a JSONLogic expression - create a child node for it
        const branchId = convertValue(item as JsonLogicValue, {
          nodes: context.nodes,
          edges: context.edges,
          parentId: nodeId,
          argIndex: expressionIndex,
          preserveStructure: context.preserveStructure,
        });

        elements.push({
          type: 'expression',
          path,
          key,
          branchId,
          startOffset: 0, // Will be calculated after formatting
          endOffset: 0,
        });

        expressionIndex++;
        return EXPR_PLACEHOLDER;
      } else {
        // Inline value - keep as-is for formatting
        return item;
      }
    }
  );

  // Format the JSON with placeholders
  const formattedJson = JSON.stringify(structureWithPlaceholders, null, 2);

  // Calculate offsets for expression placeholders in the formatted JSON
  // Note: JSON.stringify wraps strings in quotes, so we search for "{{EXPR}}"
  let searchPos = 0;
  for (const element of elements) {
    if (element.type === 'expression') {
      const placeholderPos = formattedJson.indexOf(EXPR_PLACEHOLDER_QUOTED, searchPos);
      if (placeholderPos !== -1) {
        element.startOffset = placeholderPos;
        element.endOffset = placeholderPos + EXPR_PLACEHOLDER_QUOTED.length;
        searchPos = element.endOffset;
      }
    }
  }

  // Generate expression text for collapsed view
  const expressionText = generateExpressionText(value as JsonLogicValue, 100);

  // Create the structure node
  const node: LogicNode = {
    id: nodeId,
    type: 'structure',
    position: { x: 0, y: 0 },
    data: {
      type: 'structure',
      isArray,
      formattedJson,
      elements,
      collapsed: false,
      expressionText,
      expression: value as JsonLogicValue,
      parentId: parentInfo.parentId,
      argIndex: parentInfo.argIndex,
      branchType: parentInfo.branchType,
    } as StructureNodeData,
  };

  context.nodes.push(node);

  // Add edge from parent if exists and not a branch type
  if (parentInfo.parentId && !parentInfo.branchType) {
    const edge = createArgEdge(parentInfo.parentId, nodeId, parentInfo.argIndex ?? 0);
    context.edges.push(edge);
  }

  // Add edges from structure node to expression child nodes
  // Note: branchIndex must match the handle IDs in StructureNode (0, 1, 2, ...)
  let branchIdx = 0;
  for (const element of elements) {
    if (element.type === 'expression' && element.branchId) {
      const edge = createBranchEdge(nodeId, element.branchId, branchIdx);
      context.edges.push(edge);
      branchIdx++;
    }
  }

  return nodeId;
}

/**
 * Walk through a structure (object or array) and transform values.
 * For JSONLogic expressions, the callback returns the replacement value.
 * For other values, they are walked recursively if they are objects/arrays.
 */
function walkAndCollect(
  value: unknown,
  path: string[],
  onValue: (path: string[], item: unknown, key?: string) => unknown
): unknown {
  if (Array.isArray(value)) {
    return value.map((item, index) => {
      const itemPath = [...path, String(index)];
      if (isJsonLogicExpression(item)) {
        return onValue(itemPath, item);
      } else if (typeof item === 'object' && item !== null) {
        // Recursively walk nested structures
        return walkAndCollect(item, itemPath, onValue);
      }
      return item;
    });
  }

  if (typeof value === 'object' && value !== null) {
    const result: Record<string, unknown> = {};
    for (const [key, item] of Object.entries(value)) {
      const itemPath = [...path, key];
      if (isJsonLogicExpression(item)) {
        result[key] = onValue(itemPath, item, key);
      } else if (typeof item === 'object' && item !== null) {
        // Recursively walk nested structures
        result[key] = walkAndCollect(item, itemPath, onValue);
      } else {
        result[key] = item;
      }
    }
    return result;
  }

  return value;
}
