import type { JsonLogicValue } from '../../types';
import type { ConversionContext } from './types';
import { getParentInfo } from './types';
import { createLiteralNode, createArgEdge } from '../node-factory';

// Convert a primitive value (or non-object) to a literal node
export function convertPrimitive(
  value: JsonLogicValue,
  context: ConversionContext
): string {
  const parentInfo = getParentInfo(context);
  const node = createLiteralNode(value, parentInfo);

  context.nodes.push(node);

  // Add edge from parent if exists and not a branch type
  if (parentInfo.parentId && !parentInfo.branchType) {
    const edge = createArgEdge(parentInfo.parentId, node.id, parentInfo.argIndex ?? 0);
    context.edges.push(edge);
  }

  return node.id;
}

// Convert an invalid JSONLogic object (not exactly 1 key) to a literal node
export function convertInvalidObject(
  value: JsonLogicValue,
  context: ConversionContext
): string {
  const parentInfo = getParentInfo(context);
  const node = createLiteralNode(value, {
    ...parentInfo,
  });

  // Override valueType to 'array' for invalid objects
  if (node.data.type === 'literal') {
    node.data.valueType = 'array';
  }

  context.nodes.push(node);

  // Add edge from parent if exists and not a branch type
  if (parentInfo.parentId && !parentInfo.branchType) {
    const edge = createArgEdge(parentInfo.parentId, node.id, parentInfo.argIndex ?? 0);
    context.edges.push(edge);
  }

  return node.id;
}
