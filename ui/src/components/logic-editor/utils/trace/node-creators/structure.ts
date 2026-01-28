import type {
  LogicNode,
  JsonLogicValue,
  StructureNodeData,
  StructureElement,
} from '../../../types';
import type { ExpressionNode } from '../../../types/trace';
import type { ParentInfo } from '../../converters/types';
import type { TraceContext } from '../types';
import { generateExpressionText } from '../../formatting';
import { isJsonLogicExpression, isDataStructure } from '../../type-helpers';
import { createBranchEdge, createArgEdge } from '../../node-factory';
import { findMatchingChild } from '../child-matching';

// Forward declaration for processExpressionNode and createFallbackNode
type ProcessExpressionNodeFn = (
  exprNode: ExpressionNode,
  context: TraceContext,
  parentInfo: ParentInfo,
  originalExpression?: JsonLogicValue
) => string;

type CreateFallbackNodeFn = (
  nodeId: string,
  value: JsonLogicValue,
  context: TraceContext,
  parentInfo: ParentInfo
) => void;

// Placeholder marker used in formatted JSON for expressions
const EXPR_PLACEHOLDER = '{{EXPR}}';
// The placeholder as it appears in JSON.stringify output (with quotes)
const EXPR_PLACEHOLDER_QUOTED = `"${EXPR_PLACEHOLDER}"`;

/**
 * Check if a value should be treated as an expression branch in trace conversion
 * This includes JSONLogic expressions and nested structures (when preserveStructure is enabled)
 */
function isExpressionBranch(item: unknown, preserveStructure: boolean): boolean {
  if (isJsonLogicExpression(item)) return true;
  // In preserveStructure mode, nested structures are also separate expression nodes in the trace
  if (preserveStructure && isDataStructure(item)) return true;
  return false;
}

/**
 * Walk through a structure and transform values (for trace conversion)
 */
function walkAndCollectFromTrace(
  value: unknown,
  path: string[],
  onValue: (path: string[], item: unknown, key?: string) => unknown,
  context: TraceContext
): unknown {
  if (Array.isArray(value)) {
    return value.map((item, index) => {
      const itemPath = [...path, String(index)];
      if (isExpressionBranch(item, context.preserveStructure)) {
        return onValue(itemPath, item);
      } else if (typeof item === 'object' && item !== null) {
        return walkAndCollectFromTrace(item, itemPath, onValue, context);
      }
      return item;
    });
  }

  if (typeof value === 'object' && value !== null) {
    const result: Record<string, unknown> = {};
    for (const [key, item] of Object.entries(value)) {
      const itemPath = [...path, key];
      if (isExpressionBranch(item, context.preserveStructure)) {
        result[key] = onValue(itemPath, item, key);
      } else if (typeof item === 'object' && item !== null) {
        result[key] = walkAndCollectFromTrace(item, itemPath, onValue, context);
      } else {
        result[key] = item;
      }
    }
    return result;
  }

  return value;
}

/**
 * Create a structure node for data structures with embedded JSONLogic from trace data
 */
export function createStructureNodeFromTrace(
  nodeId: string,
  expression: JsonLogicValue,
  children: ExpressionNode[],
  context: TraceContext,
  parentInfo: ParentInfo,
  processExpressionNode: ProcessExpressionNodeFn,
  createFallbackNode: CreateFallbackNodeFn
): void {
  const isArray = Array.isArray(expression);
  const elements: StructureElement[] = [];
  const usedChildIndices = new Set<number>();
  let expressionIndex = 0;

  // Build a modified structure for JSON formatting with placeholders
  const structureWithPlaceholders = walkAndCollectFromTrace(
    expression as Record<string, unknown> | unknown[],
    [],
    (path, item, key) => {
      if (isJsonLogicExpression(item)) {
        // Find matching child in trace
        const match = findMatchingChild(item as JsonLogicValue, children, usedChildIndices);
        let branchId: string;

        if (match) {
          usedChildIndices.add(match.index);
          branchId = processExpressionNode(match.child, context, {
            parentId: nodeId,
            argIndex: expressionIndex,
          });
        } else {
          // Fallback: create appropriate node based on value type
          // Use branchType to prevent createFallbackNode from adding edges (structure node handles its own edges)
          branchId = `${nodeId}-expr-${expressionIndex}`;
          createFallbackNode(branchId, item as JsonLogicValue, context, {
            parentId: nodeId,
            argIndex: expressionIndex,
            branchType: 'branch', // Prevents edge creation in fallback
          });
        }

        elements.push({
          type: 'expression',
          path,
          key,
          branchId,
          startOffset: 0,
          endOffset: 0,
        });

        expressionIndex++;
        return EXPR_PLACEHOLDER;
      }
      return item;
    },
    context
  );

  // Format the JSON with placeholders
  const formattedJson = JSON.stringify(structureWithPlaceholders, null, 2);

  // Calculate offsets for expression placeholders
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
  const expressionText = generateExpressionText(expression, 100);

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
      expression,
      parentId: parentInfo.parentId,
      argIndex: parentInfo.argIndex,
      branchType: parentInfo.branchType,
    } as StructureNodeData,
  };
  context.nodes.push(node);

  // Add edge from parent if exists and not a branch type
  if (parentInfo.parentId && !parentInfo.branchType) {
    context.edges.push(createArgEdge(parentInfo.parentId, nodeId, parentInfo.argIndex ?? 0));
  }

  // Add edges from structure node to expression child nodes
  let branchIdx = 0;
  for (const element of elements) {
    if (element.type === 'expression' && element.branchId) {
      context.edges.push(createBranchEdge(nodeId, element.branchId, branchIdx));
      branchIdx++;
    }
  }
}
