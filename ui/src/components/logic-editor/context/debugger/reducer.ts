import type { DebuggerState, DebuggerAction } from './types';

// Initial state
// currentStepIndex starts at -1 meaning "no step active" (plain visualizer look)
export const initialState: DebuggerState = {
  isActive: false,
  steps: [],
  currentStepIndex: -1,
  playbackState: 'stopped',
  playbackSpeed: 500, // Default 500ms per step
};

// Reducer
export function debuggerReducer(state: DebuggerState, action: DebuggerAction): DebuggerState {
  switch (action.type) {
    case 'INITIALIZE':
      return {
        ...state,
        isActive: action.steps.length > 0,
        steps: action.steps,
        currentStepIndex: -1, // Start at -1: no step active (plain visualizer)
        playbackState: 'stopped',
      };

    case 'PLAY':
      if (state.steps.length === 0) return state;
      // If at the end or not started, start from first step
      if (state.currentStepIndex >= state.steps.length - 1 || state.currentStepIndex < 0) {
        return { ...state, currentStepIndex: 0, playbackState: 'playing' };
      }
      return { ...state, playbackState: 'playing' };

    case 'PAUSE':
      return { ...state, playbackState: 'paused' };

    case 'STOP':
      return { ...state, playbackState: 'stopped', currentStepIndex: -1 };

    case 'STEP_FORWARD':
      // From -1 (initial), go to 0 (first step)
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
      // Allow stepping back to -1 (initial/plain visualizer state)
      if (state.currentStepIndex > -1) {
        return {
          ...state,
          currentStepIndex: state.currentStepIndex - 1,
          playbackState: state.currentStepIndex - 1 < 0 ? 'stopped' : 'paused',
        };
      }
      return { ...state, playbackState: 'stopped' };

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
      return { ...state, currentStepIndex: -1, playbackState: 'stopped' };

    default:
      return state;
  }
}
