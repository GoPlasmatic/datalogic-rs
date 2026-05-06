import { memo } from 'react';
import { Handle, Position } from '@xyflow/react';
import type { CellData } from '../types';
import { HANDLE_POSITIONS } from '../constants';

interface CellHandlesProps {
  cell: CellData;
  color: string;
}

/**
 * Renders handles for a cell's branches.
 * Handle IDs are based on cell index for stability - they don't change when other cells collapse.
 * Format: branch-{cellIndex} or branch-{cellIndex}-cond / branch-{cellIndex}-then for if/then cells
 */
export const CellHandles = memo(function CellHandles({
  cell,
  color,
}: CellHandlesProps) {
  const cellIndex = cell.index;

  return (
    <>
      {/* Condition branch handle - positioned at 30% of row height */}
      {cell.conditionBranchId && (
        <Handle
          type="source"
          position={Position.Right}
          id={`branch-${cellIndex}-cond`}
          className="cell-handle condition-handle"
          style={{ background: color, top: `${HANDLE_POSITIONS.conditionTop}px` }}
        />
      )}
      {/* Then/Yes branch handle - positioned at 70% of row height */}
      {cell.thenBranchId && (
        <Handle
          type="source"
          position={Position.Right}
          id={`branch-${cellIndex}-then`}
          className="cell-handle then-handle"
          style={{ background: '#22C55E', top: `${HANDLE_POSITIONS.thenTop}px` }}
        />
      )}
      {/* Standard single branch handle - centered vertically */}
      {cell.branchId && !cell.conditionBranchId && !cell.thenBranchId && (
        <Handle
          type="source"
          position={Position.Right}
          id={`branch-${cellIndex}`}
          className="cell-handle"
          style={{ background: color, top: `${HANDLE_POSITIONS.centeredTop}px` }}
        />
      )}
    </>
  );
});
