import { memo } from 'react';
import type { CellData } from '../types';
import { Icon } from '../utils/icons';
import { CellHandles } from './CellHandles';
import { ExpressionSyntax } from '../utils/ExpressionSyntax';
import { BRANCH_COLORS } from '../constants';

interface CellRowProps {
  cell: CellData;
  color: string;
}

export const CellRow = memo(function CellRow({
  cell,
  color,
}: CellRowProps) {
  // Check if this is an if/then cell (has condition/then branches)
  const isIfThenCell = cell.conditionBranchId || cell.thenBranchId;
  const hasAnyBranch = cell.type === 'branch' || isIfThenCell;

  // Check if this is a "Then" row (should be indented)
  const isThenRow = cell.icon === 'check';

  return (
    <div className={`vertical-cell-row ${isThenRow ? 'vertical-cell-row-then' : ''}`}>
      {cell.icon && (
        <span className="vertical-cell-row-icon">
          <Icon name={cell.icon} size={12} />
        </span>
      )}

      {/* Row label (If, Then, Else, etc.) */}
      {cell.rowLabel && (
        <span className="vertical-cell-row-label">
          {cell.rowLabel}
        </span>
      )}

      {/* Display expression - same for all cell types */}
      <span className={`vertical-cell-inline ${hasAnyBranch ? 'branch-expression' : ''}`}>
        <ExpressionSyntax text={cell.label || cell.summary?.label || '...'} />
      </span>

      {/* Handles for branch cells */}
      {hasAnyBranch && (
        <CellHandles cell={cell} color={isThenRow ? BRANCH_COLORS.yes : color} />
      )}
    </div>
  );
});
