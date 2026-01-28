import type { JsonLogicValue, LogicNode, LogicEdge } from '../../types';
import type { TracedResult, ExpressionNode } from '../../types/trace';
import type { ParentInfo } from '../converters/types';
import type { TraceConversionResult, TraceToNodesOptions, TraceContext } from './types';
import { traceIdToNodeId } from './evaluation-results';
import { determineNodeType } from './node-type';
import {
  createLiteralNodeFromTrace,
  createVariableNodeFromTrace,
  createOperatorNodeFromTrace,
  createVerticalCellNodeFromTrace,
  createIfElseNodeFromTrace,
  createStructureNodeFromTrace,
} from './node-creators';

/**
 * Main conversion function: Convert trace data to visual nodes and edges
 */
export function traceToNodes(trace: TracedResult, options: TraceToNodesOptions = {}): TraceConversionResult {
  if (!trace.expression_tree) {
    return { nodes: [], edges: [], rootId: null, traceNodeMap: new Map() };
  }

  const nodes: LogicNode[] = [];
  const edges: LogicEdge[] = [];
  const traceNodeMap: Map<string, string> = new Map();

  // Use original value if provided (preserves key ordering), otherwise parse from trace
  const rootExpression = options.originalValue ?? JSON.parse(trace.expression_tree.expression);

  processExpressionNode(trace.expression_tree, {
    nodes,
    edges,
    traceNodeMap,
    preserveStructure: options.preserveStructure ?? false,
  }, {}, rootExpression);

  return {
    nodes,
    edges,
    rootId: traceIdToNodeId(trace.expression_tree.id),
    traceNodeMap,
  };
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
  // Use original expression if provided (preserves key ordering), otherwise parse from trace
  const expression: JsonLogicValue = originalExpression ?? JSON.parse(exprNode.expression);

  // Register this node in the trace map - it maps to itself since it creates a visual node
  context.traceNodeMap.set(nodeId, nodeId);

  // Determine the type of expression and create appropriate node
  const nodeType = determineNodeType(expression, context.preserveStructure);

  switch (nodeType) {
    case 'literal':
      createLiteralNodeFromTrace(nodeId, expression, exprNode.children, context, parentInfo);
      break;
    case 'variable':
      createVariableNodeFromTrace(nodeId, expression, exprNode.children, context, parentInfo);
      break;
    case 'if':
      createIfElseNodeFromTrace(
        nodeId, expression, exprNode.children, context, parentInfo,
        processExpressionNode, createFallbackNode
      );
      break;
    case 'verticalCell':
      createVerticalCellNodeFromTrace(
        nodeId, expression, exprNode.children, context, parentInfo,
        processExpressionNode, createFallbackNode
      );
      break;
    case 'operator':
      createOperatorNodeFromTrace(
        nodeId, expression, exprNode.children, context, parentInfo,
        processExpressionNode
      );
      break;
    case 'structure':
      createStructureNodeFromTrace(
        nodeId, expression, exprNode.children, context, parentInfo,
        processExpressionNode, createFallbackNode
      );
      break;
  }

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
  // Determine the appropriate node type based on the value
  const nodeType = determineNodeType(value, context.preserveStructure);

  // Create the appropriate node type, passing empty children since we don't have trace data
  switch (nodeType) {
    case 'literal':
      createLiteralNodeFromTrace(nodeId, value, [], context, parentInfo);
      break;
    case 'variable':
      createVariableNodeFromTrace(nodeId, value, [], context, parentInfo);
      break;
    case 'if':
      createIfElseNodeFromTrace(
        nodeId, value, [], context, parentInfo,
        processExpressionNode, createFallbackNode
      );
      break;
    case 'verticalCell':
      createVerticalCellNodeFromTrace(
        nodeId, value, [], context, parentInfo,
        processExpressionNode, createFallbackNode
      );
      break;
    case 'operator':
      createOperatorNodeFromTrace(
        nodeId, value, [], context, parentInfo,
        processExpressionNode
      );
      break;
    case 'structure':
      createStructureNodeFromTrace(
        nodeId, value, [], context, parentInfo,
        processExpressionNode, createFallbackNode
      );
      break;
  }
}
