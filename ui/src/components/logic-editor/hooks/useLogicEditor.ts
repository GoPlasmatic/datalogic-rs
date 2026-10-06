import { useMemo } from 'react';
import type {
  LogicNode,
  LogicEdge,
  JsonLogicValue,
  TracedResult,
  ExecutionStep,
} from '../types';
import { convertJsonLogic } from '../utils/jsonlogic-to-nodes';
import {
  traceToNodes,
  isCompileFailedTrace,
  getTraceFailure,
  resolveFailedNodeIds,
  type TraceFailure,
} from '../utils/trace';
import { applyTreeLayout } from '../utils/layout';
import { checkDepth } from './useRecursionCheck';
import type { FlowDirection } from '../context/DirectionContextDef';

interface UseLogicEditorOptions {
  value: JsonLogicValue | null;
  evaluateWithTrace?: (logic: unknown, data: unknown) => TracedResult;
  data?: unknown;
  /** Enable templating mode (multi-key objects compile to output-shaping templates with embedded JSONLogic). */
  templating?: boolean;
  /** Diagram direction, flips the layout rankDir (default 'flow'). */
  direction?: FlowDirection;
}

interface UseLogicEditorReturn {
  nodes: LogicNode[];
  edges: LogicEdge[];
  /** Conversion / rendering error (the diagram could not be built). */
  error: string | null;
  usingTraceMode: boolean;
  steps: ExecutionStep[];
  traceNodeMap: Map<string, string>;  // Maps trace node IDs to visual node IDs
  /**
   * Engine failure reported by the trace envelope: a compile-stage error
   * (parse error, unknown operator; the diagram then comes from the static
   * converter and there are no steps) or a runtime error (steps run up to the
   * failing node). Undefined when evaluation succeeded or no trace ran.
   */
  traceError?: TraceFailure;
  /**
   * Visual node ids on the engine's failure breadcrumb (innermost first),
   * resolved through `traceNodeMap`. Undefined when there is no failure.
   */
  failedNodeIds?: Set<string>;
}

// Maximum recursion depth to prevent stack overflow
const MAX_RECURSION_DEPTH = 100;

// Root id for the static conversion. Fixed, so converting the same rule
// again (the echo of an edit) yields the same node ids.
const ROOT_NODE_ID = 'n';

const emptySteps: ExecutionStep[] = [];
const emptyTraceNodeMap: Map<string, string> = new Map();

function failedModel(error: string | null): UseLogicEditorReturn {
  return {
    nodes: [],
    edges: [],
    error,
    usingTraceMode: false,
    steps: emptySteps,
    traceNodeMap: emptyTraceNodeMap,
    traceError: undefined,
    failedNodeIds: undefined,
  };
}

/** Build the diagram (and, with a trace, the debugger's steps) for a rule. */
function buildModel(
  value: JsonLogicValue | null,
  data: unknown,
  evaluateWithTrace: UseLogicEditorOptions['evaluateWithTrace'],
  templating: boolean,
  direction: FlowDirection,
): UseLogicEditorReturn {
  try {
    // Validate recursion depth
    if (!checkDepth(value, MAX_RECURSION_DEPTH)) {
      return failedModel(`Expression exceeds maximum nesting depth of ${MAX_RECURSION_DEPTH}`);
    }

    // Engine failure from the trace envelope (compile or runtime), if any
    let failure: TraceFailure | undefined;

    // Try trace-based conversion first if available
    if (evaluateWithTrace && value) {
      try {
        const trace = evaluateWithTrace(value, data ?? {});
        failure = getTraceFailure(trace);
        if (!isCompileFailedTrace(trace)) {
          const { nodes, edges, traceNodeMap } = traceToNodes(trace, { templating, originalValue: value });
          return {
            nodes: applyTreeLayout(nodes, edges, direction),
            edges,
            error: null,
            usingTraceMode: true,
            steps: trace.steps,
            traceNodeMap,
            traceError: failure,
            failedNodeIds: failure ? resolveFailedNodeIds(trace, traceNodeMap) : undefined,
          };
        }
        // Compile-stage failure: no tree to render from, fall through to the
        // static converter and keep the failure visible.
      } catch (traceErr) {
        // Trace conversion failed, fall back to JS parsing
        console.warn('Trace conversion failed, falling back to JS:', traceErr);
      }
    }

    // Fallback to JS parsing (no execution steps)
    const { nodes, edges } = convertJsonLogic(value, { templating }, ROOT_NODE_ID);
    return {
      nodes: applyTreeLayout(nodes, edges, direction),
      edges,
      error: null,
      usingTraceMode: false,
      steps: emptySteps,
      traceNodeMap: emptyTraceNodeMap,
      traceError: failure,
      failedNodeIds: undefined,
    };
  } catch (err) {
    return failedModel(err instanceof Error ? err.message : 'Unknown error during conversion');
  }
}

export function useLogicEditor({
  value,
  evaluateWithTrace,
  data,
  templating = false,
  direction = 'flow',
}: UseLogicEditorOptions): UseLogicEditorReturn {
  // Derived during render. The rule and data are keyed by content, not
  // identity: a host may pass an equal but new object on every render, and
  // re-converting (and re-tracing) for that would be wasted work. The
  // conversion reads the parsed copies, which is also all the engine sees.
  const valueKey = JSON.stringify(value);
  const dataKey = JSON.stringify(data);
  return useMemo(
    () =>
      buildModel(
        valueKey === undefined ? null : (JSON.parse(valueKey) as JsonLogicValue | null),
        dataKey === undefined ? undefined : JSON.parse(dataKey),
        evaluateWithTrace,
        templating,
        direction,
      ),
    [valueKey, dataKey, evaluateWithTrace, templating, direction]
  );
}
