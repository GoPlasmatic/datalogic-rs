import type { ExecutionStep } from '../../types/trace';
import type { TraceFailure } from '../../utils/trace/trace-failure';

// Playback states
export type PlaybackState = 'playing' | 'paused' | 'stopped';

// Debugger state
export interface DebuggerState {
  isActive: boolean;
  steps: ExecutionStep[];
  currentStepIndex: number;
  playbackState: PlaybackState;
  playbackSpeed: number; // milliseconds per step
}

// Actions
export type DebuggerAction =
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

// Compact description of a visual node, for step lists and tooltips
export interface NodeSummary {
  /** Operator label (or 'literal' / 'object' / 'array') */
  label: string;
  /** Expression text (or the literal's value) */
  detail: string;
}

// Context value type
export interface DebuggerContextValue {
  state: DebuggerState;
  currentStep: ExecutionStep | null;
  currentNodeId: string | null;
  executedNodeIds: Set<string>;
  errorNodeIds: Set<string>; // Node IDs that encountered errors
  pathNodeIds: Set<string>; // Node IDs on the path from current node to root
  /** Trace node id (`trace-N`) -> visual node id, as produced by traceToNodes */
  traceNodeMap: Map<string, string>;
  /** Node IDs on the engine's failure breadcrumb (innermost first); empty without a failure */
  failedNodeIds: Set<string>;
  /** The innermost failed node (first of failedNodeIds), or null */
  primaryFailedNodeId: string | null;
  /** Trace-level failure (compile or runtime), or null */
  traceError: TraceFailure | null;
  /** Visual node id -> label / expression summary (for the step list) */
  nodeSummaries: Map<string, NodeSummary>;
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

// Node debug state (returned by useNodeDebugState hook)
export interface NodeDebugState {
  isCurrent: boolean;
  isExecuted: boolean;
  isPending: boolean;
  isOnPath: boolean; // Node is on the path from current node to root
  isError: boolean; // Node encountered an error during evaluation
  isFailed: boolean; // Node is on the engine's failure breadcrumb
  step: ExecutionStep | null;
}
