import { useNodeDebugState } from '../context';

/**
 * Hook that generates debug-related CSS class names for a node based on its debug state.
 * Returns a space-separated string of class names that can be added to the node's className.
 * `debug-error` marks a node whose step failed (or, at rest, a node on the
 * engine's failure breadcrumb); `debug-failed` additionally marks breadcrumb nodes.
 */
export function useDebugClassName(nodeId: string): string {
  const debugState = useNodeDebugState(nodeId);

  if (!debugState) return '';

  return [
    debugState.isCurrent && 'debug-current',
    debugState.isExecuted && 'debug-executed',
    debugState.isPending && 'debug-pending',
    debugState.isOnPath && !debugState.isCurrent && 'debug-on-path',
    debugState.isError && 'debug-error',
    debugState.isFailed && 'debug-failed',
  ].filter(Boolean).join(' ');
}
