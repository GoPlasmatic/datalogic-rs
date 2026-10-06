import type { ExecutionStep } from '../../types/trace';
import type { TraceFailure } from '../../utils/trace/trace-failure';
import { traceIdToNodeId } from '../../utils/trace/trace-ids';
import type { NodeDebugState } from './types';

/**
 * Everything a node needs to work out its own debug state at one step.
 *
 * Node components read it through `DebugNodeStore` rather than through the
 * debugger context: the context value changes on every step, which made
 * every node re-render on every step even when its own state was unchanged.
 */
export interface DebugSnapshot {
  isActive: boolean;
  currentStepIndex: number;
  stepCount: number;
  currentStep: ExecutionStep | null;
  currentNodeId: string | null;
  /** Visual node id -> the first step index that ran it. */
  firstRunAt: Map<string, number>;
  /** Visual node id -> the first step index that failed on it. */
  firstErrorAt: Map<string, number>;
  /** The current node and its ancestors. */
  pathNodeIds: Set<string>;
  failedNodeIds: Set<string>;
  primaryFailedNodeId: string | null;
  traceError: TraceFailure | null;
}

/** Visual node id of a step: the mapped node, or the trace id itself. */
export function stepNodeId(step: ExecutionStep, traceNodeMap: Map<string, string>): string {
  const traceId = traceIdToNodeId(step.node_id);
  return traceNodeMap.get(traceId) ?? traceId;
}

/**
 * When each node first ran and first failed, from one pass over the steps.
 * A node is executed at step k when it first ran before k, and in error
 * when it first failed at or before k. Recomputing those sets from step 0
 * on every step made playback quadratic in the trace length.
 */
export function indexSteps(
  steps: ExecutionStep[],
  traceNodeMap: Map<string, string>,
): { firstRunAt: Map<string, number>; firstErrorAt: Map<string, number> } {
  const firstRunAt = new Map<string, number>();
  const firstErrorAt = new Map<string, number>();
  steps.forEach((step, index) => {
    const id = stepNodeId(step, traceNodeMap);
    if (!firstRunAt.has(id)) firstRunAt.set(id, index);
    if (step.error && !firstErrorAt.has(id)) firstErrorAt.set(id, index);
  });
  return { firstRunAt, firstErrorAt };
}

/** A node's debug state at the snapshot's step, or null when it has none. */
export function computeNodeDebugState(s: DebugSnapshot, nodeId: string): NodeDebugState | null {
  const isFailed = s.failedNodeIds.has(nodeId);
  const atRest = !s.isActive || s.currentStepIndex < 0;

  // At rest (-1 = plain visualizer): only failed nodes carry debug state,
  // and the innermost one shows the trace failure.
  if (atRest) {
    if (!isFailed) return null;
    return {
      isCurrent: false,
      isExecuted: false,
      isPending: false,
      isOnPath: false,
      isError: true,
      isFailed: true,
      step: null,
      failure: s.primaryFailedNodeId === nodeId ? s.traceError : null,
    };
  }

  const isCurrent = s.currentNodeId === nodeId;
  const isExecuted = (s.firstRunAt.get(nodeId) ?? Infinity) < s.currentStepIndex;
  const isOnPath = s.pathNodeIds.has(nodeId);
  const atEnd = s.currentStepIndex >= s.stepCount - 1;
  const isError = (s.firstErrorAt.get(nodeId) ?? Infinity) <= s.currentStepIndex || (isFailed && atEnd);
  const isPending = !isCurrent && !isExecuted && !isOnPath;

  return {
    isCurrent,
    isExecuted,
    isPending,
    isOnPath,
    isError,
    isFailed,
    step: isCurrent ? s.currentStep : null,
    failure: null,
  };
}

function sameNodeState(a: NodeDebugState | null, b: NodeDebugState | null): boolean {
  if (a === b) return true;
  if (!a || !b) return false;
  return (
    a.isCurrent === b.isCurrent &&
    a.isExecuted === b.isExecuted &&
    a.isPending === b.isPending &&
    a.isOnPath === b.isOnPath &&
    a.isError === b.isError &&
    a.isFailed === b.isFailed &&
    a.step === b.step &&
    a.failure === b.failure
  );
}

export interface DebugNodeStore {
  subscribe: (listener: () => void) => () => void;
  /**
   * The node's state, as the same object for as long as it is unchanged,
   * so `useSyncExternalStore` re-renders only the nodes a step affects.
   */
  getNodeState: (nodeId: string) => NodeDebugState | null;
  setSnapshot: (snapshot: DebugSnapshot) => void;
}

export function createDebugNodeStore(initial: DebugSnapshot): DebugNodeStore {
  let snapshot = initial;
  const listeners = new Set<() => void>();
  const cache = new Map<string, { snapshot: DebugSnapshot; state: NodeDebugState | null }>();

  return {
    subscribe(listener) {
      listeners.add(listener);
      return () => {
        listeners.delete(listener);
      };
    },
    getNodeState(nodeId) {
      const cached = cache.get(nodeId);
      if (cached && cached.snapshot === snapshot) return cached.state;
      const next = computeNodeDebugState(snapshot, nodeId);
      const state = cached && sameNodeState(cached.state, next) ? cached.state : next;
      cache.set(nodeId, { snapshot, state });
      return state;
    },
    setSnapshot(next) {
      if (next === snapshot) return;
      snapshot = next;
      for (const listener of listeners) listener();
    },
  };
}
