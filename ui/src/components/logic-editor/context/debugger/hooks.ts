import { useContext, useMemo } from 'react';
import { DebuggerContext } from './context';
import type { NodeDebugState, DebuggerContextValue } from './types';

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
  const context = useContext(DebuggerContext);

  return useMemo(() => {
    if (!context) return null;

    const isFailed = context.failedNodeIds.has(nodeId);
    const { isActive, currentStepIndex, steps } = context.state;
    const atRest = !isActive || currentStepIndex < 0;

    // At rest (-1 = plain visualizer): only failed nodes carry debug state
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
      };
    }

    const isCurrent = context.currentNodeId === nodeId;
    const isExecuted = context.executedNodeIds.has(nodeId);
    const isOnPath = context.pathNodeIds.has(nodeId);
    const atEnd = currentStepIndex >= steps.length - 1;
    const isError = context.errorNodeIds.has(nodeId) || (isFailed && atEnd);
    const isPending = !isCurrent && !isExecuted && !isOnPath;

    return {
      isCurrent,
      isExecuted,
      isPending,
      isOnPath,
      isError,
      isFailed,
      step: isCurrent ? context.currentStep : null,
    };
  }, [context, nodeId]);
}
