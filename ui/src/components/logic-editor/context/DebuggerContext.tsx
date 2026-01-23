import {
  createContext,
  useContext,
  useReducer,
  useEffect,
  useMemo,
  useCallback,
  type ReactNode,
} from 'react';
import type { ExecutionStep } from '../types/trace';
import type { LogicNode } from '../types';

// Playback states
type PlaybackState = 'playing' | 'paused' | 'stopped';

// Debugger state
interface DebuggerState {
  isActive: boolean;
  steps: ExecutionStep[];
  currentStepIndex: number;
  playbackState: PlaybackState;
  playbackSpeed: number; // milliseconds per step
}

// Actions
type DebuggerAction =
  | { type: 'INITIALIZE'; steps: ExecutionStep[] }
  | { type: 'PLAY' }
  | { type: 'PAUSE' }
  | { type: 'STOP' }
  | { type: 'STEP_FORWARD' }
  | { type: 'STEP_BACKWARD' }
  | { type: 'GO_TO_STEP'; index: number }
  | { type: 'SET_SPEED'; speed: number }
  | { type: 'RESET' }
  | { type: 'AUTO_STEP_FORWARD' };

// Initial state
const initialState: DebuggerState = {
  isActive: false,
  steps: [],
  currentStepIndex: 0,
  playbackState: 'stopped',
  playbackSpeed: 500, // Default 500ms per step
};

// Reducer
function debuggerReducer(state: DebuggerState, action: DebuggerAction): DebuggerState {
  switch (action.type) {
    case 'INITIALIZE':
      return {
        ...state,
        isActive: action.steps.length > 0,
        steps: action.steps,
        currentStepIndex: 0,
        playbackState: 'stopped',
      };

    case 'PLAY':
      if (state.steps.length === 0) return state;
      // If at the end, reset to start
      if (state.currentStepIndex >= state.steps.length - 1) {
        return { ...state, currentStepIndex: 0, playbackState: 'playing' };
      }
      return { ...state, playbackState: 'playing' };

    case 'PAUSE':
      return { ...state, playbackState: 'paused' };

    case 'STOP':
      return { ...state, playbackState: 'stopped', currentStepIndex: 0 };

    case 'STEP_FORWARD':
      if (state.currentStepIndex < state.steps.length - 1) {
        return {
          ...state,
          currentStepIndex: state.currentStepIndex + 1,
          playbackState: 'paused',
        };
      }
      return { ...state, playbackState: 'paused' };

    case 'AUTO_STEP_FORWARD':
      // Used during auto-play - doesn't change playback state
      if (state.currentStepIndex < state.steps.length - 1) {
        return {
          ...state,
          currentStepIndex: state.currentStepIndex + 1,
        };
      }
      // At end, pause playback
      return { ...state, playbackState: 'paused' };

    case 'STEP_BACKWARD':
      if (state.currentStepIndex > 0) {
        return {
          ...state,
          currentStepIndex: state.currentStepIndex - 1,
          playbackState: 'paused',
        };
      }
      return { ...state, playbackState: 'paused' };

    case 'GO_TO_STEP': {
      const clampedIndex = Math.max(0, Math.min(action.index, state.steps.length - 1));
      return {
        ...state,
        currentStepIndex: clampedIndex,
        playbackState: 'paused',
      };
    }

    case 'SET_SPEED':
      return { ...state, playbackSpeed: action.speed };

    case 'RESET':
      return { ...state, currentStepIndex: 0, playbackState: 'stopped' };

    default:
      return state;
  }
}

// Context value type
interface DebuggerContextValue {
  state: DebuggerState;
  currentStep: ExecutionStep | null;
  currentNodeId: string | null;
  executedNodeIds: Set<string>;
  pathNodeIds: Set<string>;  // Node IDs on the path from current node to root
  // Controls
  play: () => void;
  pause: () => void;
  stop: () => void;
  reset: () => void;
  stepForward: () => void;
  stepBackward: () => void;
  goToStep: (index: number) => void;
  setSpeed: (ms: number) => void;
}

// Create context
const DebuggerContext = createContext<DebuggerContextValue | null>(null);

// Provider props
interface DebuggerProviderProps {
  children: ReactNode;
  steps: ExecutionStep[];
  traceNodeMap: Map<string, string>;  // Maps trace node IDs to visual node IDs
  nodes: LogicNode[];  // For building parent map
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

  // Current step
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

// Hook to get full debugger context
// eslint-disable-next-line react-refresh/only-export-components
export function useDebuggerContext() {
  const context = useContext(DebuggerContext);
  if (!context) {
    throw new Error('useDebuggerContext must be used within a DebuggerProvider');
  }
  return context;
}

// Hook to get debug state for a specific node
interface NodeDebugState {
  isCurrent: boolean;
  isExecuted: boolean;
  isPending: boolean;
  isOnPath: boolean;  // Node is on the path from current node to root
  step: ExecutionStep | null;
}

// eslint-disable-next-line react-refresh/only-export-components
export function useNodeDebugState(nodeId: string): NodeDebugState | null {
  const context = useContext(DebuggerContext);

  return useMemo(() => {
    if (!context || !context.state.isActive) return null;

    const isCurrent = context.currentNodeId === nodeId;
    const isExecuted = context.executedNodeIds.has(nodeId);
    const isOnPath = context.pathNodeIds.has(nodeId);
    const isPending = !isCurrent && !isExecuted && !isOnPath;

    return {
      isCurrent,
      isExecuted,
      isPending,
      isOnPath,
      step: isCurrent ? context.currentStep : null,
    };
  }, [context, nodeId]);
}
