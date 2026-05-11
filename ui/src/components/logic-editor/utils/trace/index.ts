// Main entry points
export { traceToNodes } from './trace-to-nodes';
export { buildEvaluationResultsFromTrace, traceIdToNodeId } from './evaluation-results';

// Types
export type {
  TraceConversionResult,
  TraceToNodesOptions,
  TraceContext,
  ValueType,
  NodeType,
  ChildMatch,
} from './types';

// Child matching utilities (exported for potential reuse)
export { findMatchingChild, getNextUnusedChild } from './child-matching';

// Node type determination
export { determineNodeType } from './node-type';

// Inline mapping utility
export { mapInlinedChildren } from './inline-mapping';
