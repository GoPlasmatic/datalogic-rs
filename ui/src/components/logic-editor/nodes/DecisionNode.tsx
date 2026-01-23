import { memo } from 'react';
import { Handle, Position } from '@xyflow/react';
import type { DecisionNodeData } from '../types';
import { BRANCH_COLORS } from '../constants';
import { useIsHandleConnected } from '../context';
import { useDebugClassName } from '../hooks';
import { NodeDebugBubble } from './shared';
import { Icon } from '../utils/icons';
import { ExpressionSyntax } from '../utils/ExpressionSyntax';

// Decision node color (amber)
const DECISION_COLOR = '#F59E0B';

// Handle positioning - relative positions within each row (32px rows)
const HANDLE_OFFSET = 16; // Center of 32px row

interface DecisionNodeProps {
  id: string;
  data: DecisionNodeData;
  selected?: boolean;
}

export const DecisionNode = memo(function DecisionNode({
  id,
  data,
  selected,
}: DecisionNodeProps) {
  const hasLeftConnection = useIsHandleConnected(id, 'left');
  const debugClassName = useDebugClassName(id);

  return (
    <div
      className={`decision-node ${selected ? 'selected' : ''} ${debugClassName}`}
      style={{
        borderColor: DECISION_COLOR,
        backgroundColor: `${DECISION_COLOR}10`,
      }}
    >
      <NodeDebugBubble nodeId={id} position="top" />

      {/* Input handle from left (for LR layout) - only show if connected */}
      {hasLeftConnection && (
        <Handle
          type="target"
          position={Position.Left}
          id="left"
          style={{ background: DECISION_COLOR, top: '50%' }}
        />
      )}

      {/* Header with icon and title */}
      <div className="decision-node-header" style={{ backgroundColor: DECISION_COLOR }}>
        <Icon name="diamond" size={14} />
        <span className="decision-node-title">If / Then</span>
      </div>

      {/* Body with condition and branch rows */}
      <div className="decision-node-body">
        {/* Condition row - with handle inside the row */}
        <div className="decision-row" style={{ position: 'relative' }}>
          <span className="decision-row-label">if</span>
          <div className="decision-row-content">
            <ExpressionSyntax text={data.conditionText} />
          </div>
          {/* Condition handle positioned within this row */}
          {data.isConditionComplex && (
            <Handle
              type="source"
              position={Position.Right}
              id="branch-0"
              style={{ background: '#64748B', top: `${HANDLE_OFFSET}px` }}
            />
          )}
        </div>

        {/* Then row - with handle inside the row */}
        <div className="decision-row decision-row-branch" style={{ position: 'relative' }}>
          <span className="decision-row-label" style={{ color: BRANCH_COLORS.yes }}>
            <Icon name="check" size={12} /> then
          </span>
          {/* Yes handle positioned within this row */}
          <Handle
            type="source"
            position={Position.Right}
            id={data.isConditionComplex ? 'branch-1' : 'branch-0'}
            style={{ background: BRANCH_COLORS.yes, top: `${HANDLE_OFFSET}px` }}
          />
        </div>

        {/* Else row - with handle inside the row */}
        <div className="decision-row decision-row-branch" style={{ position: 'relative' }}>
          <span className="decision-row-label" style={{ color: BRANCH_COLORS.no }}>
            <Icon name="x" size={12} /> else
          </span>
          {/* No handle positioned within this row */}
          <Handle
            type="source"
            position={Position.Right}
            id={data.isConditionComplex ? 'branch-2' : 'branch-1'}
            style={{ background: BRANCH_COLORS.no, top: `${HANDLE_OFFSET}px` }}
          />
        </div>
      </div>
    </div>
  );
});
