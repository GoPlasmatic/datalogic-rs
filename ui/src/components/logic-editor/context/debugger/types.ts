import type { ExecutionStep } from '../../types/trace';

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

// Context value type
export interface DebuggerContextValue {
  state: DebuggerState;
  currentStep: ExecutionStep | null;
  currentNodeId: string | null;
  executedNodeIds: Set<string>;
  pathNodeIds: Set<string>; // Node IDs on the path from current node to root
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
  step: ExecutionStep | null;
}
