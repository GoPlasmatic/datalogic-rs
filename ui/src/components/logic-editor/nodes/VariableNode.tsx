import { memo } from 'react';
import { Box, Database, Search } from 'lucide-react';
import type { VariableNodeData } from '../types';
import { CATEGORY_COLORS } from '../types';
import { useDebugClassName } from '../hooks';
import { NodeInputHandles, NodeDebugBubble } from './shared';

interface VariableNodeProps {
  id: string;
  data: VariableNodeData;
  selected?: boolean;
}

export const VariableNode = memo(function VariableNode({
  id,
  data,
  selected,
}: VariableNodeProps) {
  const color = CATEGORY_COLORS.variable;
  const debugClassName = useDebugClassName(id);

  // Get the icon component based on operator type
  const OperatorIcon = data.operator === 'var' ? Box : data.operator === 'val' ? Database : Search;
  const operatorLabel = data.operator;

  return (
    <div
      className={`variable-node ${selected ? 'selected' : ''} ${debugClassName}`}
      style={{
        borderColor: color,
        backgroundColor: `${color}20`,
      }}
    >
      <NodeDebugBubble nodeId={id} position="top" />
      <NodeInputHandles nodeId={id} color={color} />

      <div className="variable-node-content">
        <span className="variable-operator" style={{ color, display: 'inline-flex', alignItems: 'center', gap: '4px' }}>
          <OperatorIcon size={14} />
          {operatorLabel}
        </span>
        <span className="variable-path">{data.path || '(empty)'}</span>
      </div>

      {data.defaultValue !== undefined && (
        <div className="variable-default">
          <span className="variable-default-label">default:</span>
          <span className="variable-default-value">
            {JSON.stringify(data.defaultValue)}
          </span>
        </div>
      )}
    </div>
  );
});
