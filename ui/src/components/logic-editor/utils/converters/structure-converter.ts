import { v4 as uuidv4 } from 'uuid';
import type { JsonLogicValue, StructureNodeData, StructureElement, LogicNode } from '../../types';
import type { ConversionContext, ConverterFn } from './types';
import { getParentInfo } from './types';
import { createArgEdge, createBranchEdge } from '../node-factory';
import { isJsonLogicExpression } from '../type-helpers';
import { generateExpressionText } from '../formatting';
import { formatStructureWithPlaceholders } from './structure-paths';

/**
 * Convert a data structure (object or array with potential JSONLogic expressions)
 * to a structure node that displays formatted JSON with linked expression branches.
 *
 * Only embedded expressions become elements (and child nodes); every literal
 * field stays inside `data.expression`, which the serializer walks, substituting
 * each element's child at its recorded path.
 */
export function convertStructure(
  value: Record<string, unknown> | unknown[],
  context: ConversionContext,
  convertValue: ConverterFn
): string {
  const parentInfo = getParentInfo(context);
  const nodeId = uuidv4();
  const isArray = Array.isArray(value);

  // Collect expression elements in document order
  const collected: StructureElement[] = [];
  let expressionIndex = 0;

  walkStructure(value, [], (path, item, key) => {
    // This is a JSONLogic expression - create a child node for it
    // `branchType: 'branch'` keeps the child from adding its own arg edge:
    // this node pushes one branch edge per expression element below, and
    // edge ids are `${source}-${target}`, so both would collide.
    const branchId = convertValue(item as JsonLogicValue, {
      nodes: context.nodes,
      edges: context.edges,
      parentId: nodeId,
      argIndex: expressionIndex,
      branchType: 'branch',
      templating: context.templating,
    });

    collected.push({
      type: 'expression',
      path,
      key,
      branchId,
      startOffset: 0,
      endOffset: 0,
    });

    expressionIndex++;
  });

  // Format the JSON with placeholders and compute the element offsets
  const { formattedJson, elements } = formatStructureWithPlaceholders(
    value as JsonLogicValue,
    collected
  );

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
 * Walk a structure (object or array) in document order and call `onExpression`
 * for every embedded JSONLogic expression with its path. Nested plain
 * structures are descended into; expressions are not.
 */
function walkStructure(
  value: unknown,
  path: string[],
  onExpression: (path: string[], item: unknown, key?: string) => void
): void {
  if (Array.isArray(value)) {
    value.forEach((item, index) => {
      const itemPath = [...path, String(index)];
      if (isJsonLogicExpression(item)) {
        onExpression(itemPath, item);
      } else if (typeof item === 'object' && item !== null) {
        walkStructure(item, itemPath, onExpression);
      }
    });
    return;
  }

  if (typeof value === 'object' && value !== null) {
    for (const [key, item] of Object.entries(value)) {
      const itemPath = [...path, key];
      if (isJsonLogicExpression(item)) {
        onExpression(itemPath, item, key);
      } else if (typeof item === 'object' && item !== null) {
        walkStructure(item, itemPath, onExpression);
      }
    }
  }
}
