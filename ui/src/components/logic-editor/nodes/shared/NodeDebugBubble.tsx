import { memo, useContext } from 'react';
import { useNodeDebugState } from '../../context';
import { DebuggerContext } from '../../context/debugger/context';
import { DebugInfoBubble } from '../DebugInfoBubble';

interface NodeDebugBubbleProps {
  nodeId: string;
  position?: 'top' | 'right' | 'bottom';
}

/**
 * Wrapper component that conditionally renders a DebugInfoBubble
 * when the node is the current step in debugging, or, at rest, when it is
 * the innermost node on the engine's failure breadcrumb.
 */
export const NodeDebugBubble = memo(function NodeDebugBubble({
  nodeId,
  position = 'top',
}: NodeDebugBubbleProps) {
  const debugState = useNodeDebugState(nodeId);
  const debugger_ = useContext(DebuggerContext);

  if (!debugState) return null;

  if (debugState.isCurrent && debugState.step) {
    return <DebugInfoBubble step={debugState.step} position={position} />;
  }

  // At rest: surface the trace failure on the innermost failed node
  if (
    debugState.isFailed &&
    !debugState.isCurrent &&
    debugger_ &&
    debugger_.state.currentStepIndex < 0 &&
    debugger_.primaryFailedNodeId === nodeId &&
    debugger_.traceError
  ) {
    return <DebugInfoBubble failure={debugger_.traceError} position={position} />;
  }

  return null;
});
