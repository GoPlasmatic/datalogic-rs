import { describe, expect, it } from 'vitest';
import { debuggerReducer, initialState } from '../reducer';
import type { DebuggerState } from '../types';
import type { ExecutionStep } from '../../../types/trace';

function steps(n: number): ExecutionStep[] {
  return Array.from({ length: n }, (_, i) => ({ step_id: i, node_id: i, context: {}, result: i, error: null }));
}

function loaded(n = 3, overrides: Partial<DebuggerState> = {}): DebuggerState {
  return { ...debuggerReducer(initialState, { type: 'INITIALIZE', steps: steps(n) }), ...overrides };
}

describe('debuggerReducer', () => {
  it('initializes at rest, before the first step', () => {
    const state = loaded(3);
    expect(state).toMatchObject({ isActive: true, currentStepIndex: -1, playbackState: 'stopped' });
    expect(debuggerReducer(initialState, { type: 'INITIALIZE', steps: [] }).isActive).toBe(false);
  });

  it('keeps the playback speed across a new trace', () => {
    const state = debuggerReducer(loaded(3, { playbackSpeed: 900 }), { type: 'INITIALIZE', steps: steps(2) });
    expect(state.playbackSpeed).toBe(900);
  });

  it('steps forward from rest to the first step and stops at the last', () => {
    let state = loaded(2);
    state = debuggerReducer(state, { type: 'STEP_FORWARD' });
    expect(state).toMatchObject({ currentStepIndex: 0, playbackState: 'paused' });
    state = debuggerReducer(state, { type: 'STEP_FORWARD' });
    state = debuggerReducer(state, { type: 'STEP_FORWARD' });
    expect(state.currentStepIndex).toBe(1);
  });

  it('steps back to rest, stopping playback there', () => {
    let state = loaded(2, { currentStepIndex: 1, playbackState: 'paused' });
    state = debuggerReducer(state, { type: 'STEP_BACKWARD' });
    expect(state).toMatchObject({ currentStepIndex: 0, playbackState: 'paused' });
    state = debuggerReducer(state, { type: 'STEP_BACKWARD' });
    expect(state).toMatchObject({ currentStepIndex: -1, playbackState: 'stopped' });
    state = debuggerReducer(state, { type: 'STEP_BACKWARD' });
    expect(state.currentStepIndex).toBe(-1);
  });

  it('plays from the start when at rest or at the end, otherwise from where it is', () => {
    expect(debuggerReducer(loaded(3), { type: 'PLAY' })).toMatchObject({ currentStepIndex: 0, playbackState: 'playing' });
    expect(debuggerReducer(loaded(3, { currentStepIndex: 2 }), { type: 'PLAY' }).currentStepIndex).toBe(0);
    expect(debuggerReducer(loaded(3, { currentStepIndex: 1 }), { type: 'PLAY' }).currentStepIndex).toBe(1);
    expect(debuggerReducer(initialState, { type: 'PLAY' })).toBe(initialState);
  });

  it('auto-steps during playback and pauses at the end', () => {
    let state = loaded(2, { currentStepIndex: 0, playbackState: 'playing' });
    state = debuggerReducer(state, { type: 'AUTO_STEP_FORWARD' });
    expect(state).toMatchObject({ currentStepIndex: 1, playbackState: 'playing' });
    state = debuggerReducer(state, { type: 'AUTO_STEP_FORWARD' });
    expect(state).toMatchObject({ currentStepIndex: 1, playbackState: 'paused' });
  });

  it('clamps GO_TO_STEP into the trace', () => {
    expect(debuggerReducer(loaded(3), { type: 'GO_TO_STEP', index: 99 }).currentStepIndex).toBe(2);
    expect(debuggerReducer(loaded(3), { type: 'GO_TO_STEP', index: -5 }).currentStepIndex).toBe(0);
  });

  it('pauses, stops, resets and sets the speed', () => {
    const playing = loaded(3, { currentStepIndex: 1, playbackState: 'playing' });
    expect(debuggerReducer(playing, { type: 'PAUSE' }).playbackState).toBe('paused');
    expect(debuggerReducer(playing, { type: 'STOP' })).toMatchObject({ currentStepIndex: -1, playbackState: 'stopped' });
    expect(debuggerReducer(playing, { type: 'RESET' })).toMatchObject({ currentStepIndex: -1, playbackState: 'stopped' });
    expect(debuggerReducer(playing, { type: 'SET_SPEED', speed: 200 }).playbackSpeed).toBe(200);
  });
});
