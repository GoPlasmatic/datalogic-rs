// Main entry points
export { traceToNodes } from './trace-to-nodes';
export { traceIdToNodeId } from './trace-ids';

// Trace-level failures (compile / runtime errors in the envelope)
export {
  isCompileFailedTrace,
  getTraceFailure,
  formatTraceFailure,
  traceFailureType,
  resolveFailedNodeIds,
  type TraceFailure,
} from './trace-failure';

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
export {
  findMatchingChild,
  getNextUnusedChild,
  matchOperandsToChildren,
  normalizeExpression,
  mayHaveTraceNode,
} from './child-matching';

// Node type determination
export { determineNodeType } from './node-type';

// Inline mapping utility
export { mapInlinedChildren } from './inline-mapping';
