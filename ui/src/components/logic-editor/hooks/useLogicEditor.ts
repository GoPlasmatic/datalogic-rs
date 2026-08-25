import { useState, useEffect, useMemo, useRef } from 'react';
import type {
  LogicNode,
  LogicEdge,
  JsonLogicValue,
  TracedResult,
  ExecutionStep,
} from '../types';
import { jsonLogicToNodes } from '../utils/jsonlogic-to-nodes';
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

const emptySteps: ExecutionStep[] = [];
const emptyTraceNodeMap: Map<string, string> = new Map();

export function useLogicEditor({
  value,
  evaluateWithTrace,
  data,
  templating = false,
  direction = 'flow',
}: UseLogicEditorOptions): UseLogicEditorReturn {
  const [nodes, setNodes] = useState<LogicNode[]>([]);
  const [edges, setEdges] = useState<LogicEdge[]>([]);
  const [error, setError] = useState<string | null>(null);
  const [usingTraceMode, setUsingTraceMode] = useState(false);
  const [steps, setSteps] = useState<ExecutionStep[]>(emptySteps);
  const [traceNodeMap, setTraceNodeMap] = useState<Map<string, string>>(emptyTraceNodeMap);
  const [traceError, setTraceError] = useState<TraceFailure | undefined>(undefined);
  const [failedNodeIds, setFailedNodeIds] = useState<Set<string> | undefined>(undefined);
  const lastExternalValueRef = useRef<string>('');
  const lastDataRef = useRef<string>('');
  const lastHadTraceRef = useRef<boolean>(false);
  const lastTemplatingRef = useRef<boolean>(false);
  const lastDirectionRef = useRef<FlowDirection>('flow');

  // Convert JSONLogic to nodes when value changes from outside
  /* eslint-disable react-hooks/set-state-in-effect -- Derived state computation from value/data props */
  useEffect(() => {
    const valueStr = JSON.stringify(value);
    const dataStr = JSON.stringify(data);
    const hasTrace = !!evaluateWithTrace;

    // Re-process if value, data, trace availability, templating, or direction changed
    if (
      valueStr === lastExternalValueRef.current &&
      dataStr === lastDataRef.current &&
      hasTrace === lastHadTraceRef.current &&
      templating === lastTemplatingRef.current &&
      direction === lastDirectionRef.current
    ) {
      return;
    }

    const rememberInputs = () => {
      lastExternalValueRef.current = valueStr;
      lastDataRef.current = dataStr;
      lastHadTraceRef.current = hasTrace;
      lastTemplatingRef.current = templating;
      lastDirectionRef.current = direction;
    };

    const clearTraceState = () => {
      setSteps(emptySteps);
      setTraceNodeMap(emptyTraceNodeMap);
      setUsingTraceMode(false);
    };

    try {
      // Validate recursion depth
      if (!checkDepth(value, MAX_RECURSION_DEPTH)) {
        setError(`Expression exceeds maximum nesting depth of ${MAX_RECURSION_DEPTH}`);
        setNodes([]);
        setEdges([]);
        clearTraceState();
        setTraceError(undefined);
        setFailedNodeIds(undefined);
        rememberInputs();
        return;
      }

      // Engine failure from the trace envelope (compile or runtime), if any
      let failure: TraceFailure | undefined;

      // Try trace-based conversion first if available
      if (evaluateWithTrace && value) {
        try {
          const trace = evaluateWithTrace(value, data ?? {});
          failure = getTraceFailure(trace);
          if (!isCompileFailedTrace(trace)) {
            const { nodes: newNodes, edges: newEdges, traceNodeMap: newTraceNodeMap } = traceToNodes(trace, { templating, originalValue: value });
            const layoutedNodes = applyTreeLayout(newNodes, newEdges, direction);
            setNodes(layoutedNodes);
            setEdges(newEdges);
            setSteps(trace.steps);
            setTraceNodeMap(newTraceNodeMap);
            setUsingTraceMode(true);
            setError(null);
            setTraceError(failure);
            setFailedNodeIds(failure ? resolveFailedNodeIds(trace, newTraceNodeMap) : undefined);
            rememberInputs();
            return;
          }
          // Compile-stage failure: no tree to render from, fall through to the
          // static converter and keep the failure visible.
        } catch (traceErr) {
          // Trace conversion failed, fall back to JS parsing
          console.warn('Trace conversion failed, falling back to JS:', traceErr);
        }
      }

      // Fallback to JS parsing (no execution steps)
      const { nodes: newNodes, edges: newEdges } = jsonLogicToNodes(value, { templating });
      const layoutedNodes = applyTreeLayout(newNodes, newEdges, direction);
      setNodes(layoutedNodes);
      setEdges(newEdges);
      clearTraceState();
      setError(null);
      setTraceError(failure);
      setFailedNodeIds(undefined);
    } catch (err) {
      const errorMessage = err instanceof Error ? err.message : 'Unknown error during conversion';
      setError(errorMessage);
      setNodes([]);
      setEdges([]);
      clearTraceState();
      setTraceError(undefined);
      setFailedNodeIds(undefined);
    }
    rememberInputs();
  }, [value, data, evaluateWithTrace, templating, direction]);
  /* eslint-enable react-hooks/set-state-in-effect */

  // Memoize return value to maintain stable identity
  return useMemo(
    () => ({
      nodes,
      edges,
      error,
      usingTraceMode,
      steps,
      traceNodeMap,
      traceError,
      failedNodeIds,
    }),
    [nodes, edges, error, usingTraceMode, steps, traceNodeMap, traceError, failedNodeIds]
  );
}
