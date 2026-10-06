import { describe, expect, it } from 'vitest';
import {
  computeNodeDebugState,
  createDebugNodeStore,
  indexSteps,
  stepNodeId,
  type DebugSnapshot,
} from '../node-store';
import type { ExecutionStep } from '../../../types/trace';

// trace ids 0..3 map to visual nodes; 3 is inlined into node b
const traceNodeMap = new Map([
  ['trace-0', 'root'],
  ['trace-1', 'a'],
  ['trace-2', 'b'],
  ['trace-3', 'b'],
]);
const parents = new Map([
  ['a', 'root'],
  ['b', 'root'],
]);

function step(id: number, node: number, error: string | null = null): ExecutionStep {
  return { step_id: id, node_id: node, context: {}, result: id, error };
}

const STEPS: ExecutionStep[] = [step(0, 1), step(1, 3), step(2, 2, 'boom'), step(3, 1), step(4, 0)];
const FAILED = new Set(['b', 'root']);

function snapshotAt(index: number): DebugSnapshot {
  const currentStep = index >= 0 ? STEPS[index] : null;
  const currentNodeId = currentStep ? stepNodeId(currentStep, traceNodeMap) : null;
  const pathNodeIds = new Set<string>();
  for (let id = currentNodeId ?? undefined; id; id = parents.get(id)) pathNodeIds.add(id);
  return {
    isActive: true,
    currentStepIndex: index,
    stepCount: STEPS.length,
    currentStep,
    currentNodeId,
    ...indexSteps(STEPS, traceNodeMap),
    pathNodeIds,
    failedNodeIds: FAILED,
    primaryFailedNodeId: 'b',
    traceError: null,
  };
}

/** The former implementation: rescan the trace from step 0. */
function naive(index: number, nodeId: string) {
  const executed = new Set<string>();
  for (let i = 0; i < index; i++) executed.add(stepNodeId(STEPS[i], traceNodeMap));
  const errors = new Set<string>();
  for (let i = 0; i <= index; i++) if (STEPS[i].error) errors.add(stepNodeId(STEPS[i], traceNodeMap));
  return {
    isExecuted: executed.has(nodeId),
    isError: errors.has(nodeId) || (FAILED.has(nodeId) && index >= STEPS.length - 1),
  };
}

describe('debugger node store', () => {
  it('matches a rescan of the trace at every step', () => {
    for (let index = 0; index < STEPS.length; index++) {
      for (const nodeId of ['root', 'a', 'b']) {
        const state = computeNodeDebugState(snapshotAt(index), nodeId)!;
        expect({ isExecuted: state.isExecuted, isError: state.isError }).toEqual(naive(index, nodeId));
      }
    }
  });

  it('marks the current node, its path and pending nodes', () => {
    const at1 = snapshotAt(1); // trace-3 -> b
    expect(computeNodeDebugState(at1, 'b')).toMatchObject({ isCurrent: true, isOnPath: true, step: STEPS[1] });
    expect(computeNodeDebugState(at1, 'root')).toMatchObject({ isCurrent: false, isOnPath: true, isPending: false });
    expect(computeNodeDebugState(at1, 'a')).toMatchObject({ isExecuted: true, isPending: false });
  });

  it('at rest, reports only failed nodes and the failure on the innermost one', () => {
    const rest: DebugSnapshot = { ...snapshotAt(-1), traceError: 'boom' };
    expect(computeNodeDebugState(rest, 'a')).toBeNull();
    expect(computeNodeDebugState(rest, 'b')).toMatchObject({ isFailed: true, isError: true, failure: rest.traceError });
    expect(computeNodeDebugState(rest, 'root')?.failure).toBeNull();
  });

  it('returns the same object while a node is unaffected, and notifies on change', () => {
    const store = createDebugNodeStore(snapshotAt(3));
    let notified = 0;
    const unsubscribe = store.subscribe(() => notified++);

    const rootBefore = store.getNodeState('root');
    const aBefore = store.getNodeState('a');
    store.setSnapshot(snapshotAt(3)); // a new snapshot with the same step
    expect(notified).toBe(1);
    expect(store.getNodeState('root')).toBe(rootBefore);
    expect(store.getNodeState('a')).toBe(aBefore); // still current, on the same step

    store.setSnapshot(snapshotAt(4));
    expect(store.getNodeState('root')).not.toBe(rootBefore); // root became current

    unsubscribe();
    store.setSnapshot(snapshotAt(0));
    expect(notified).toBe(2);
  });
});
