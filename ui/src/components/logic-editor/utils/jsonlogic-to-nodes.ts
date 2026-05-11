import type {
  JsonLogicValue,
  LogicNode,
  LogicEdge,
  ConversionResult,
} from '../types';
import { convertValue } from './converters';

// Options for converting JSONLogic to nodes
export interface JsonLogicToNodesOptions {
  /** Enable templating mode (multi-key objects compile to output-shaping templates with embedded JSONLogic). */
  templating?: boolean;
}

// Main conversion function
export function jsonLogicToNodes(
  expr: JsonLogicValue | null,
  options: JsonLogicToNodesOptions = {}
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
  });

  return { nodes, edges, rootId };
}
