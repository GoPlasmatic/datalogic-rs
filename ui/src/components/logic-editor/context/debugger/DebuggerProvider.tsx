import {
  useReducer,
  useEffect,
  useMemo,
  useCallback,
  type ReactNode,
} from 'react';
import type { ExecutionStep } from '../../types/trace';
import type { LogicNode } from '../../types';
import { DebuggerContext } from './context';
import { debuggerReducer, initialState } from './reducer';

// Provider props
interface DebuggerProviderProps {
  children: ReactNode;
  steps: ExecutionStep[];
  traceNodeMap: Map<string, string>; // Maps trace node IDs to visual node IDs
  nodes: LogicNode[]; // For building parent map
}

// Provider component
export function DebuggerProvider({ children, steps, traceNodeMap, nodes }: DebuggerProviderProps) {
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
  const currentNodeId = useMemo(() => {
    if (!currentStep) return null;
    const traceId = `trace-${currentStep.node_id}`;
    // Use mapping to find visual node, fallback to trace ID itself
    return traceNodeMap.get(traceId) ?? traceId;
  }, [currentStep, traceNodeMap]);

  // Set of executed node IDs (all steps up to current) - use mapping for each
  const executedNodeIds = useMemo(() => {
    const ids = new Set<string>();
    for (let i = 0; i < state.currentStepIndex; i++) {
      const step = state.steps[i];
      if (step) {
        const traceId = `trace-${step.node_id}`;
        const visualId = traceNodeMap.get(traceId) ?? traceId;
        ids.add(visualId);
      }
    }
    return ids;
  }, [state.steps, state.currentStepIndex, traceNodeMap]);

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
      executedNodeIds,
      pathNodeIds,
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
      executedNodeIds,
      pathNodeIds,
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

  return <DebuggerContext.Provider value={value}>{children}</DebuggerContext.Provider>;
}
