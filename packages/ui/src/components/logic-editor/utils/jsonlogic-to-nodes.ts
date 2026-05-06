import type {
  JsonLogicValue,
  LogicNode,
  LogicEdge,
  ConversionResult,
} from '../types';
import { convertValue } from './converters';

// Options for converting JSONLogic to nodes
export interface JsonLogicToNodesOptions {
  /** Enable structure preserve mode for JSON templates with embedded JSONLogic */
  preserveStructure?: boolean;
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
    preserveStructure: options.preserveStructure,
  });

  return { nodes, edges, rootId };
}
