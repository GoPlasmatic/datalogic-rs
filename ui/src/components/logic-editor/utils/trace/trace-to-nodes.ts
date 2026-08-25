import type { JsonLogicValue, LogicNode, LogicEdge } from '../../types';
import type { TracedResult, ExpressionNode } from '../../types/trace';
import type { ParentInfo } from '../converters/types';
import type { TraceConversionResult, TraceToNodesOptions, TraceContext } from './types';
import { traceIdToNodeId } from './trace-ids';
import { determineNodeType } from './node-type';
import { isCompileFailedTrace } from './trace-failure';
import { parseTraceExpression } from './child-matching';
import { isVariableOperatorName } from '../converters/variable-cells';
import { isStaticVariableExpression } from './node-creators/variable';
import {
  createLiteralNodeFromTrace,
  createVariableNodeFromTrace,
  createVerticalCellNodeFromTrace,
  createIfElseNodeFromTrace,
  createSwitchNodeFromTrace,
  createStructureNodeFromTrace,
} from './node-creators';

const EMPTY_RESULT = (): TraceConversionResult => ({
  nodes: [],
  edges: [],
  rootId: null,
  traceNodeMap: new Map(),
});

/**
 * Main conversion function: Convert trace data to visual nodes and edges.
 *
 * A compile-failed envelope (placeholder tree with an empty expression) has
 * nothing to render and yields the empty result; callers fall back to the
 * static converter and surface `trace.structured_error` / `trace.error`.
 */
export function traceToNodes(trace: TracedResult, options: TraceToNodesOptions = {}): TraceConversionResult {
  if (!trace.expression_tree || isCompileFailedTrace(trace)) {
    return EMPTY_RESULT();
  }

  // Use original value if provided (preserves key ordering), otherwise parse from trace
  const rootExpression: JsonLogicValue | undefined =
    options.originalValue ?? (parseTraceExpression(trace.expression_tree) as JsonLogicValue | null) ?? undefined;
  if (rootExpression === undefined) {
    return EMPTY_RESULT();
  }

  const nodes: LogicNode[] = [];
  const edges: LogicEdge[] = [];
  const traceNodeMap: Map<string, string> = new Map();

  processExpressionNode(trace.expression_tree, {
    nodes,
    edges,
    traceNodeMap,
    templating: options.templating ?? false,
  }, {}, rootExpression);

  resolveHiddenTraceIds(trace, traceNodeMap);

  return {
    nodes,
    edges,
    rootId: traceIdToNodeId(trace.expression_tree.id),
    traceNodeMap,
  };
}

/**
 * Some compiled nodes keep their operator arguments out of the expression
 * tree (`missing` / `missing_some` with dynamic paths are leaves) yet still
 * record steps for them. Compile-time ids are assigned post-order, so the
 * nearest ancestor of an unknown id is the smallest tree id greater than it.
 * Map every step / breadcrumb id the tree does not list onto that ancestor's
 * visual node so the debugger never points at a node that does not exist.
 */
function resolveHiddenTraceIds(trace: TracedResult, traceNodeMap: Map<string, string>): void {
  const knownIds: number[] = [];
  const collect = (node: ExpressionNode) => {
    knownIds.push(node.id);
    for (const child of node.children ?? []) collect(child);
  };
  collect(trace.expression_tree);
  knownIds.sort((a, b) => a - b);
  const rootVisualId = traceNodeMap.get(traceIdToNodeId(trace.expression_tree.id));

  const resolve = (id: number) => {
    const traceId = traceIdToNodeId(id);
    if (traceNodeMap.has(traceId)) return;
    const ancestor = knownIds.find((known) => known > id);
    const visualId = ancestor !== undefined ? traceNodeMap.get(traceIdToNodeId(ancestor)) : undefined;
    const target = visualId ?? rootVisualId;
    if (target) traceNodeMap.set(traceId, target);
  };

  for (const step of trace.steps ?? []) resolve(step.node_id);
  for (const id of trace.structured_error?.node_ids ?? []) resolve(id);
}

/**
 * Process a single expression node from the trace
 * originalExpression can be provided to preserve key ordering (used for root and structure nodes)
 */
function processExpressionNode(
  exprNode: ExpressionNode,
  context: TraceContext,
  parentInfo: ParentInfo = {},
  originalExpression?: JsonLogicValue
): string {
  const nodeId = traceIdToNodeId(exprNode.id);
  // Use original expression if provided (preserves key ordering), otherwise parse from trace.
  // An unparseable expression string (unescaped quotes in a path) renders as its raw text.
  const expression: JsonLogicValue =
    originalExpression ?? ((parseTraceExpression(exprNode) ?? exprNode.expression) as JsonLogicValue);

  // Register this node in the trace map - it maps to itself since it creates a visual node
  context.traceNodeMap.set(nodeId, nodeId);

  createNodeForExpression(nodeId, expression, exprNode.children ?? [], context, parentInfo);

  return nodeId;
}

/**
 * Create a fallback node when no trace match is found
 * This properly handles all node types (operators, variables, structures, etc.)
 */
function createFallbackNode(
  nodeId: string,
  value: JsonLogicValue,
  context: TraceContext,
  parentInfo: ParentInfo
): void {
  // No trace data for this subtree: create the node with empty children
  createNodeForExpression(nodeId, value, [], context, parentInfo);
}

/**
 * Dispatch to the node creator matching the expression's shape. Shared by the
 * trace-backed path (children from the engine tree) and the fallback path
 * (no children).
 */
function createNodeForExpression(
  nodeId: string,
  expression: JsonLogicValue,
  children: ExpressionNode[],
  context: TraceContext,
  parentInfo: ParentInfo
): void {
  const nodeType = determineNodeType(expression, context.templating);

  switch (nodeType) {
    case 'literal':
      createLiteralNodeFromTrace(nodeId, expression, children, context, parentInfo);
      break;
    case 'operator': {
      const obj = expression as Record<string, unknown>;
      const op = Object.keys(obj)[0];
      // A computed path (expression where a path segment is expected) is wired
      // as ordinary operator arguments, like the static converter does.
      if (isVariableOperatorName(op) && isStaticVariableExpression(op, obj[op] as JsonLogicValue)) {
        createVariableNodeFromTrace(
          nodeId, expression, children, context, parentInfo,
          processExpressionNode, createFallbackNode
        );
      } else if (op === 'if' || op === '?:') {
        createIfElseNodeFromTrace(
          nodeId, expression, children, context, parentInfo,
          processExpressionNode, createFallbackNode
        );
      } else if (op === 'switch' || op === 'match') {
        createSwitchNodeFromTrace(
          nodeId, expression, children, context, parentInfo,
          processExpressionNode, createFallbackNode
        );
      } else {
        // Every other operator (any arity) is a cells-based node, like the
        // static converter's convertOperator.
        createVerticalCellNodeFromTrace(
          nodeId, expression, children, context, parentInfo,
          processExpressionNode, createFallbackNode
        );
      }
      break;
    }
    case 'structure':
      createStructureNodeFromTrace(
        nodeId, expression, children, context, parentInfo,
        processExpressionNode, createFallbackNode
      );
      break;
  }
}
