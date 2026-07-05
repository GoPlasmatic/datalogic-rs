import { memo } from 'react';
import type { CellData } from '../types';
import { Icon } from '../utils/icons';
import { CellHandles } from './CellHandles';
import { ExpressionSyntax } from '../utils/ExpressionSyntax';
import { BRANCH_COLORS } from '../constants';
import { pillTypeForText, pillForValueType } from '../utils/nodeShape';

interface CellRowProps {
  cell: CellData;
  color: string;
}

/** A simple operand -> a type-coloured pill; anything complex -> syntax text. */
function Operand({ text }: { text: string }) {
  const pill = pillTypeForText(text);
  if (pill === 'expr') {
    return (
      <span className="vertical-cell-inline">
        <ExpressionSyntax text={text} />
      </span>
    );
  }
  return <span className={`dl-pill dl-pill-${pill}`}>{text}</span>;
}

export const CellRow = memo(function CellRow({ cell, color }: CellRowProps) {
  const hasAnyBranch =
    cell.type === 'branch' || !!cell.conditionBranchId || !!cell.thenBranchId;
  // A "Then" row (should be indented).
  const isThenRow = cell.icon === 'check';

  // Leading icon + label — identical across every row kind.
  const prefix = (
    <>
      {cell.icon && (
        <span className="vertical-cell-row-icon">
          <Icon name={cell.icon} size={12} />
        </span>
      )}
      {cell.rowLabel && (
        <span className="vertical-cell-row-label">{cell.rowLabel}</span>
      )}
    </>
  );

  // Branch cell — wires out to a child sub-expression node. Instead of repeating
  // the child's logic (the child node already shows it), render a compact,
  // type-tinted "extends to child" chip. If/Then/Else rows read as coloured rails;
  // the full expression stays available on hover via the title.
  if (cell.type !== 'editable' && hasAnyBranch) {
    const isCondition = cell.icon === 'diamond';
    const railClass =
      cell.icon === 'check'
        ? 'branch-rail-then'
        : cell.icon === 'x'
          ? 'branch-rail-else'
          : isCondition
            ? 'branch-rail-when'
            : '';
    const childPill = pillForValueType(cell.summary?.valueType);
    return (
      <div className={`vertical-cell-row branch-rail ${railClass} ${isThenRow ? 'vertical-cell-row-then' : ''}`}>
        {isCondition ? (
          // Flowchart decision diamond — "if" / "elif" chained down the spine.
          <span className="dl-cond-diamond" aria-hidden="true">
            <svg viewBox="0 0 44 44">
              <path d="M22,2 L42,22 L22,42 L2,22 Z" />
            </svg>
            <span className="dl-cond-label">{cell.rowLabel}</span>
          </span>
        ) : (
          prefix
        )}
        <span
          className={`dl-pill dl-pill-${childPill} dl-child-link`}
          title={cell.summary?.label || cell.label || ''}
        >
          <span className="dl-child-glyph" aria-hidden="true">⤷</span>
        </span>
        <CellHandles cell={cell} color={isThenRow ? BRANCH_COLORS.yes : color} />
      </div>
    );
  }

  // Editable (var path / val scope / literal operand) or inline value -> pill.
  const text =
    cell.type === 'editable'
      ? cell.label || String(cell.value ?? cell.placeholder ?? '...')
      : cell.label || cell.summary?.label || '...';
  return (
    <div className="vertical-cell-row">
      {prefix}
      <Operand text={text} />
    </div>
  );
});
