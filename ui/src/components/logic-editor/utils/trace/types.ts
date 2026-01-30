import type { ConversionResult, JsonLogicValue, LogicNode, LogicEdge } from '../../types';
import type { ExpressionNode } from '../../types/trace';

// Extended result type that includes trace-to-visual node mapping
export interface TraceConversionResult extends ConversionResult {
  traceNodeMap: Map<string, string>; // trace-{id} -> visual node ID
}

// Options for trace conversion
export interface TraceToNodesOptions {
  /** Enable structure preserve mode for JSON templates with embedded JSONLogic */
  preserveStructure?: boolean;
  /** Original expression value - used to preserve key ordering in structure nodes */
  originalValue?: JsonLogicValue;
}

// Internal context passed through trace processing
export interface TraceContext {
  nodes: LogicNode[];
  edges: LogicEdge[];
  traceNodeMap: Map<string, string>;
  preserveStructure: boolean;
}

// Value type for evaluation results
export type ValueType = 'boolean' | 'number' | 'string' | 'null' | 'array' | 'object' | 'undefined';

// Node type determination result
export type NodeType = 'operator' | 'literal' | 'structure';

// Match result for child finding
export interface ChildMatch {
  child: ExpressionNode;
  index: number;
}
