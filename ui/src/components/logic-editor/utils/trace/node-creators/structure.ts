import type {
  LogicNode,
  JsonLogicValue,
  StructureNodeData,
  StructureElement,
} from '../../../types';
import type { ExpressionNode } from '../../../types/trace';
import type { ParentInfo } from '../../converters/types';
import type { TraceContext, ChildMatch } from '../types';
import { generateExpressionText } from '../../formatting';
import { isJsonLogicExpression } from '../../type-helpers';
import { createBranchEdge, createArgEdge } from '../../node-factory';
import { matchOperandsToChildren, unmatchedChildren } from '../child-matching';
import { mapInlinedChildren } from '../inline-mapping';
import { traceIdToNodeId } from '../trace-ids';
import { exprMarkerFactory, resolveExprMarkers } from '../../converters/structure-paths';

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


/**
 * Create a structure node for data structures with embedded JSONLogic from trace data.
 *
 * Mirrors the static structure converter: the structure renders as one node
 * whose JSONLogic expressions (at any nesting depth) become child nodes. The
 * engine tree, however, gives every nested object / array its own node, so
 * nested structures are matched to their trace child, folded onto this node
 * (their steps highlight the structure), and their expressions are matched
 * against that child's children.
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
  let expressionIndex = 0;
  // One distinct marker per expression slot, so the offset pass below is an
  // exact lookup rather than an ordered scan for a shared token that the
  // user's own data could also contain.
  const marker = exprMarkerFactory(expression);
  const markers: string[] = [];

  // Render one embedded expression as a child node (trace child or fallback)
  const renderExpression = (
    item: JsonLogicValue,
    path: string[],
    key: string | undefined,
    match: ChildMatch | null
  ): string => {
    let branchId: string;
    if (match) {
      branchId = processExpressionNode(match.child, context, {
        parentId: nodeId,
        argIndex: expressionIndex,
        branchType: 'branch', // structure node draws its own branch edges
      }, item);
    } else {
      branchId = `${nodeId}-expr-${expressionIndex}`;
      createFallbackNode(branchId, item, context, {
        parentId: nodeId,
        argIndex: expressionIndex,
        branchType: 'branch', // prevents edge creation in fallback
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
    markers.push(marker(expressionIndex));
    expressionIndex++;
    return markers[markers.length - 1];
  };

  // Walk one structure level, matching its entries against the trace
  // children in scope, and return the structure with expression placeholders.
  const walk = (
    value: Record<string, unknown> | unknown[],
    path: string[],
    scope: ExpressionNode[]
  ): unknown => {
    const entries: { key?: string; item: unknown }[] = Array.isArray(value)
      ? value.map((item) => ({ item }))
      : Object.entries(value).map(([key, item]) => ({ key, item }));
    const matches = matchOperandsToChildren(
      entries.map((e) => e.item as JsonLogicValue),
      scope,
      context.templating
    );

    const rendered = entries.map(({ key, item }, i) => {
      const itemPath = [...path, key ?? String(i)];
      const match = matches[i];
      if (isJsonLogicExpression(item)) {
        return renderExpression(item as JsonLogicValue, itemPath, key, match);
      }
      if (item !== null && typeof item === 'object') {
        // Nested structure: stays inline, its trace node folds onto this node
        if (match) context.traceNodeMap.set(traceIdToNodeId(match.child.id), nodeId);
        return walk(item as Record<string, unknown> | unknown[], itemPath, match ? match.child.children ?? [] : []);
      }
      return item;
    });

    // Trace children no entry claimed fold into this node
    mapInlinedChildren(unmatchedChildren(scope, matches), nodeId, context.traceNodeMap);

    if (Array.isArray(value)) return rendered;
    const result: Record<string, unknown> = {};
    entries.forEach(({ key }, i) => {
      result[key as string] = rendered[i];
    });
    return result;
  };

  const structureWithPlaceholders = walk(
    expression as Record<string, unknown> | unknown[],
    [],
    children
  );

  // Swap the per-slot markers back to the canonical placeholder and take
  // each expression element's span from the same pass.
  const { formattedJson, spans } = resolveExprMarkers(
    JSON.stringify(structureWithPlaceholders, null, 2),
    markers
  );
  elements.forEach((element, slot) => {
    const span = spans[slot];
    if (element.type === 'expression' && span) {
      element.startOffset = span.startOffset;
      element.endOffset = span.endOffset;
    }
  });

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
