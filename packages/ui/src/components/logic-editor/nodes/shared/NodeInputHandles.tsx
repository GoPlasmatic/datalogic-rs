import { memo } from 'react';
import { Handle, Position } from '@xyflow/react';
import { useIsHandleConnected } from '../../context';

interface NodeInputHandlesProps {
  nodeId: string;
  color: string;
}

/**
 * Renders input handles (top and left) for a node.
 * Only shows handles if they have connections.
 */
export const NodeInputHandles = memo(function NodeInputHandles({
  nodeId,
  color,
}: NodeInputHandlesProps) {
  const hasTopConnection = useIsHandleConnected(nodeId, 'top');
  const hasLeftConnection = useIsHandleConnected(nodeId, 'left');

  return (
    <>
      {/* Input handle from top (for vertical parent-child connections) - only show if connected */}
      {hasTopConnection && (
        <Handle
          type="target"
          position={Position.Top}
          id="top"
          style={{ background: color }}
        />
      )}
      {/* Input handle from left (for horizontal parent-child connections) - only show if connected */}
      {hasLeftConnection && (
        <Handle
          type="target"
          position={Position.Left}
          id="left"
          style={{ background: color, top: '50%' }}
        />
      )}
    </>
  );
});
