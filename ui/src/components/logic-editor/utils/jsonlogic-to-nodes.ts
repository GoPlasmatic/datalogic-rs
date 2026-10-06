import type {
  JsonLogicValue,
  LogicNode,
  LogicEdge,
  ConversionResult,
} from '../types';
import { v4 as uuidv4 } from 'uuid';
import { convertValue } from './converters';

// Options for converting JSONLogic to nodes
export interface JsonLogicToNodesOptions {
  /** Enable templating mode (multi-key objects compile to output-shaping templates with embedded JSONLogic). */
  templating?: boolean;
}

/**
 * Convert a JSONLogic expression to nodes and edges.
 *
 * Node ids are a fresh uuid for the root and `<parent id>.<slot>` below it,
 * so ids from separate calls never collide.
 */
export function jsonLogicToNodes(
  expr: JsonLogicValue | null,
  options: JsonLogicToNodesOptions = {}
): ConversionResult {
  return convertJsonLogic(expr, options, uuidv4());
}

/**
 * `jsonLogicToNodes` with a caller-chosen root id. The same expression and
 * root id always produce the same node ids, which is what lets the editor
 * keep its selection and canvas across a re-conversion.
 */
export function convertJsonLogic(
  expr: JsonLogicValue | null,
  options: JsonLogicToNodesOptions,
  rootNodeId: string
): ConversionResult {
  if (expr === null || expr === undefined) {
    return { nodes: [], edges: [], rootId: null };
  }

  const nodes: LogicNode[] = [];
  const edges: LogicEdge[] = [];

  const rootId = convertValue(expr, {
    nodes,
    edges,
    templating: options.templating,
    rootId: rootNodeId,
  });

  return { nodes, edges, rootId };
}
