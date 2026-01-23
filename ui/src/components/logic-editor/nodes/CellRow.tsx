import { memo, useCallback } from 'react';
import type { CellData } from '../types';
import { Icon } from '../utils/icons';
import { CellHandles } from './CellHandles';
import { ExpressionSyntax } from '../utils/ExpressionSyntax';
import { CollapseToggleButton } from './shared';
import { BRANCH_COLORS } from '../constants';

interface CellRowProps {
  cell: CellData;
  collapsedIndices: number[];
  color: string;
  onToggleCollapse: (cellIndex: number, e: React.MouseEvent) => void;
}

export const CellRow = memo(function CellRow({
  cell,
  collapsedIndices,
  color,
  onToggleCollapse,
}: CellRowProps) {
  const isCellCollapsed = collapsedIndices.includes(cell.index);

  // Check if this is an if/then cell (has condition/then branches)
  const isIfThenCell = cell.conditionBranchId || cell.thenBranchId;
  const hasAnyBranch = cell.type === 'branch' || isIfThenCell;
  const isExpanded = hasAnyBranch && !isCellCollapsed;

  const handleToggle = useCallback(
    (e: React.MouseEvent) => {
      onToggleCollapse(cell.index, e);
    },
    [cell.index, onToggleCollapse]
  );

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

      {/* Toggle button for branch cells */}
      {hasAnyBranch && (
        <CollapseToggleButton
          isCollapsed={isCellCollapsed}
          onClick={handleToggle}
          variant="cell"
        />
      )}

      {/* Handles for expanded branches */}
      {isExpanded && (
        <CellHandles cell={cell} color={isThenRow ? BRANCH_COLORS.yes : color} />
      )}
    </div>
  );
});
