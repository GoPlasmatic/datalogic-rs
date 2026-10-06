import {
  useReducer,
  useEffect,
  useLayoutEffect,
  useMemo,
  useCallback,
  useState,
  type ReactNode,
} from 'react';
import type { ExecutionStep } from '../../types/trace';
import type { LogicNode } from '../../types';
import type { TraceFailure } from '../../utils/trace/trace-failure';
import { formatValue } from '../../utils/formatting';
import { DebuggerContext, DebugNodeStoreContext } from './context';
import { debuggerReducer, initialState } from './reducer';
import type { NodeSummary } from './types';
import { createDebugNodeStore, indexSteps, stepNodeId, type DebugSnapshot } from './node-store';

// Layout effect in the browser (the node store updates before paint), plain
// effect on the server, where layout effects warn in React 18.
const useIsomorphicLayoutEffect = typeof window === 'undefined' ? useEffect : useLayoutEffect;

// Provider props
interface DebuggerProviderProps {
  children: ReactNode;
  steps: ExecutionStep[];
  traceNodeMap: Map<string, string>; // Maps trace node IDs to visual node IDs
  nodes: LogicNode[]; // For building parent map
  /** Visual node ids on the engine's failure breadcrumb (innermost first), optional */
  failedNodeIds?: Set<string>;
  /** Trace-level failure (compile or runtime) to show on the failed node, optional */
  traceError?: TraceFailure;
}

const EMPTY_FAILED: Set<string> = new Set();

/** Label / expression summary of a visual node for step lists. */
function summarizeNode(node: LogicNode): NodeSummary {
  const data = node.data;
  if (data.type === 'operator') {
    return { label: data.label ?? data.operator, detail: data.expressionText ?? '' };
  }
  if (data.type === 'structure') {
    return { label: data.isArray ? 'array' : 'object', detail: data.expressionText ?? '' };
  }
  if (data.type === 'literal') {
    return { label: 'literal', detail: formatValue(data.value) };
  }
  return { label: node.type ?? 'node', detail: '' };
}

// Provider component
export function DebuggerProvider({
  children,
  steps,
  traceNodeMap,
  nodes,
  failedNodeIds = EMPTY_FAILED,
  traceError,
}: DebuggerProviderProps) {
  const [state, dispatch] = useReducer(debuggerReducer, initialState);

  // Initialize with steps when they change
  useEffect(() => {
    dispatch({ type: 'INITIALIZE', steps });
  }, [steps]);

  // Auto-advance during playback
  useEffect(() => {
    if (state.playbackState !== 'playing') return;

    const timer = setInterval(() => {
      dispatch({ type: 'AUTO_STEP_FORWARD' });
    }, state.playbackSpeed);

    return () => clearInterval(timer);
  }, [state.playbackState, state.playbackSpeed]);

  // Current step (null when at -1 = initial/plain visualizer state)
  const currentStep = useMemo(() => {
    if (state.steps.length === 0 || state.currentStepIndex < 0) return null;
    return state.steps[state.currentStepIndex] ?? null;
  }, [state.steps, state.currentStepIndex]);

  // Current node ID (formatted for React Flow) - use mapping to resolve inlined nodes
  const currentNodeId = useMemo(
    () => (currentStep ? stepNodeId(currentStep, traceNodeMap) : null),
    [currentStep, traceNodeMap]
  );

  // When each node first ran and first failed: computed once per trace, so
  // a step answers "executed?" and "in error?" without rescanning the trace.
  const stepIndex = useMemo(() => indexSteps(state.steps, traceNodeMap), [state.steps, traceNodeMap]);

  // Build parent map from nodes for path highlighting
  const parentMap = useMemo(() => {
    const map = new Map<string, string>();
    for (const node of nodes) {
      if (node.data.parentId) {
        map.set(node.id, node.data.parentId);
      }
    }
    return map;
  }, [nodes]);

  // Node summaries for the step list
  const nodeSummaries = useMemo(() => {
    const map = new Map<string, NodeSummary>();
    for (const node of nodes) map.set(node.id, summarizeNode(node));
    return map;
  }, [nodes]);

  // Compute path from current node to root
  const pathNodeIds = useMemo(() => {
    const path = new Set<string>();
    if (!currentNodeId) return path;

    let nodeId: string | undefined = currentNodeId;
    while (nodeId) {
      path.add(nodeId);
      nodeId = parentMap.get(nodeId);
    }
    return path;
  }, [currentNodeId, parentMap]);

  const primaryFailedNodeId = useMemo(() => {
    const first = failedNodeIds.values().next();
    return first.done ? null : first.value;
  }, [failedNodeIds]);

  // Per-node state for node components, through a store they subscribe to
  // node by node (see useNodeDebugState).
  const snapshot = useMemo<DebugSnapshot>(
    () => ({
      isActive: state.isActive,
      currentStepIndex: state.currentStepIndex,
      stepCount: state.steps.length,
      currentStep,
      currentNodeId,
      firstRunAt: stepIndex.firstRunAt,
      firstErrorAt: stepIndex.firstErrorAt,
      pathNodeIds,
      failedNodeIds,
      primaryFailedNodeId,
      traceError: traceError ?? null,
    }),
    [
      state.isActive,
      state.currentStepIndex,
      state.steps.length,
      currentStep,
      currentNodeId,
      stepIndex,
      pathNodeIds,
      failedNodeIds,
      primaryFailedNodeId,
      traceError,
    ]
  );
  const [nodeStore] = useState(() => createDebugNodeStore(snapshot));
  useIsomorphicLayoutEffect(() => {
    nodeStore.setSnapshot(snapshot);
  }, [nodeStore, snapshot]);

  // Control callbacks
  const play = useCallback(() => dispatch({ type: 'PLAY' }), []);
  const pause = useCallback(() => dispatch({ type: 'PAUSE' }), []);
  const stop = useCallback(() => dispatch({ type: 'STOP' }), []);
  const reset = useCallback(() => dispatch({ type: 'RESET' }), []);
  const stepForward = useCallback(() => dispatch({ type: 'STEP_FORWARD' }), []);
  const stepBackward = useCallback(() => dispatch({ type: 'STEP_BACKWARD' }), []);
  const goToStep = useCallback((index: number) => dispatch({ type: 'GO_TO_STEP', index }), []);
  const setSpeed = useCallback((speed: number) => dispatch({ type: 'SET_SPEED', speed }), []);

  const value = useMemo(
    () => ({
      state,
      currentStep,
      currentNodeId,
      pathNodeIds,
      traceNodeMap,
      failedNodeIds,
      primaryFailedNodeId,
      traceError: traceError ?? null,
      nodeSummaries,
      play,
      pause,
      stop,
      reset,
      stepForward,
      stepBackward,
      goToStep,
      setSpeed,
    }),
    [
      state,
      currentStep,
      currentNodeId,
      pathNodeIds,
      traceNodeMap,
      failedNodeIds,
      primaryFailedNodeId,
      traceError,
      nodeSummaries,
      play,
      pause,
      stop,
      reset,
      stepForward,
      stepBackward,
      goToStep,
      setSpeed,
    ]
  );

  return (
    <DebuggerContext.Provider value={value}>
      <DebugNodeStoreContext.Provider value={nodeStore}>{children}</DebugNodeStoreContext.Provider>
    </DebuggerContext.Provider>
  );
}
