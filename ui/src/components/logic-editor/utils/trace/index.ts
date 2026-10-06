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

// Trace node placement by the engine's JSON Pointers
export {
  TraceSources,
  resolvePointer,
  matchOperandsToChildren,
  unmatchedChildren,
  enclosingNode,
} from './pointer-matching';

// Node type determination
export { determineNodeType } from './node-type';

// Inline mapping utility
export { mapInlinedChildren } from './inline-mapping';
