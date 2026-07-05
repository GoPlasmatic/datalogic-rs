import { memo } from 'react';
import { Handle, Position } from '@xyflow/react';
import { useIsHandleConnected, useIsFlowDirection } from '../../context';

interface NodeInputHandlesProps {
  nodeId: string;
  color: string;
}

/**
 * Renders this node's link to its parent/consumer (its result). Handle IDs are
 * kept as 'left'/'top' for backwards compatibility with the edge model. In
 * 'flow' the parent is on the right, so this is a source on the RIGHT; in
 * 'hierarchy' the parent is on the left, so it's a target on the LEFT. Only
 * shown when connected.
 */
export const NodeInputHandles = memo(function NodeInputHandles({
  nodeId,
  color,
}: NodeInputHandlesProps) {
  const isFlow = useIsFlowDirection();
  const hasTopConnection = useIsHandleConnected(nodeId, 'top');
  const hasLeftConnection = useIsHandleConnected(nodeId, 'left');

  return (
    <>
      {/* Vertical output (rarely used) */}
      {hasTopConnection && (
        <Handle
          type="source"
          position={Position.Top}
          id="top"
          style={{ background: color }}
        />
      )}
      {/* Link to the parent/consumer — the side follows the flow direction so the
          result exits toward the root (right in 'flow', left in 'hierarchy'). */}
      {hasLeftConnection && (
        <Handle
          type={isFlow ? 'source' : 'target'}
          position={isFlow ? Position.Right : Position.Left}
          id="left"
          style={{ background: color, top: '50%' }}
        />
      )}
    </>
  );
});
