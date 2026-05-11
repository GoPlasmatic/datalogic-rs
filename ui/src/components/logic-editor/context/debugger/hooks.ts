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
 * Returns null if debugger is not active
 */
export function useNodeDebugState(nodeId: string): NodeDebugState | null {
  const context = useContext(DebuggerContext);

  return useMemo(() => {
    // No debug state when debugger is inactive or at initial step (-1 = plain visualizer)
    if (!context || !context.state.isActive || context.state.currentStepIndex < 0) return null;

    const isCurrent = context.currentNodeId === nodeId;
    const isExecuted = context.executedNodeIds.has(nodeId);
    const isOnPath = context.pathNodeIds.has(nodeId);
    const isError = context.errorNodeIds.has(nodeId);
    const isPending = !isCurrent && !isExecuted && !isOnPath;

    return {
      isCurrent,
      isExecuted,
      isPending,
      isOnPath,
      isError,
      step: isCurrent ? context.currentStep : null,
    };
  }, [context, nodeId]);
}
