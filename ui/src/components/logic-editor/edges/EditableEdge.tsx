/**
 * Editable Edge Component
 *
 * A simple custom edge for the visual logic editor.
 * The + button functionality has been moved to operator nodes directly.
 */

import { memo } from 'react';
import { BaseEdge, getBezierPath, type EdgeProps } from '@xyflow/react';
import './edges.css';

export const EditableEdge = memo(function EditableEdge({
  sourceX,
  sourceY,
  targetX,
  targetY,
  sourcePosition,
  targetPosition,
  style = {},
  markerEnd,
}: EdgeProps) {
  const [edgePath] = getBezierPath({
    sourceX,
    sourceY,
    sourcePosition,
    targetX,
    targetY,
    targetPosition,
  });

  return <BaseEdge path={edgePath} markerEnd={markerEnd} style={style} />;
});

export default EditableEdge;
