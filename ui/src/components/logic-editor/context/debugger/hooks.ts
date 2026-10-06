import { useCallback, useContext, useSyncExternalStore } from 'react';
import { DebuggerContext, DebugNodeStoreContext } from './context';
import type { NodeDebugState, DebuggerContextValue } from './types';

const noSubscription = () => () => {};

/**
 * Hook to get full debugger context
 * Must be used within a DebuggerProvider
 */
export function useDebuggerContext(): DebuggerContextValue {
  const context = useContext(DebuggerContext);
  if (!context) {
    throw new Error('useDebuggerContext must be used within a DebuggerProvider');
  }
  return context;
}

/**
 * Hook to get debug state for a specific node
 * Returns null if debugger is not active, except for nodes on the engine's
 * failure breadcrumb, which report an error state even at rest so the
 * failing node is visible before stepping.
 */
export function useNodeDebugState(nodeId: string): NodeDebugState | null {
  const store = useContext(DebugNodeStoreContext);
  // Subscribes to this node's state alone: a step re-renders only the nodes
  // whose state it changes (see DebugNodeStore.getNodeState).
  const getState = useCallback(() => (store ? store.getNodeState(nodeId) : null), [store, nodeId]);
  return useSyncExternalStore(store ? store.subscribe : noSubscription, getState, getState);
}
