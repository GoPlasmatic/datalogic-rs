import { memo } from 'react';
import { useNodeDebugState } from '../../context';
import { DebugInfoBubble } from '../DebugInfoBubble';

interface NodeDebugBubbleProps {
  nodeId: string;
  position?: 'top' | 'right' | 'bottom';
}

/**
 * Wrapper component that conditionally renders a DebugInfoBubble
 * when the node is the current step in debugging.
 */
export const NodeDebugBubble = memo(function NodeDebugBubble({
  nodeId,
  position = 'top',
}: NodeDebugBubbleProps) {
  const debugState = useNodeDebugState(nodeId);

  if (!debugState?.isCurrent || !debugState.step) {
    return null;
  }

  return <DebugInfoBubble step={debugState.step} position={position} />;
});
