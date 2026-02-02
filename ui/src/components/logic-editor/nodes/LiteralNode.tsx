import { memo } from 'react';
import type { LiteralNodeData } from '../types';
import { CATEGORY_COLORS } from '../types';
import { LITERAL_TYPE_ICONS, Icon } from '../utils/icons';
import { formatValue } from '../utils/formatting';
import { useDebugClassName } from '../hooks';
import { NodeInputHandles, NodeDebugBubble } from './shared';

interface LiteralNodeProps {
  id: string;
  data: LiteralNodeData;
  selected?: boolean;
}

export const LiteralNode = memo(function LiteralNode({
  id,
  data,
  selected,
}: LiteralNodeProps) {
  const color = CATEGORY_COLORS.literal;
  const typeIcon = LITERAL_TYPE_ICONS[data.valueType];
  const debugClassName = useDebugClassName(id);

  return (
    <div
      className={`dl-node literal-node ${selected ? 'selected' : ''} ${debugClassName}`}
      style={{
        borderColor: color,
        backgroundColor: `${color}20`,
      }}
    >
      <NodeDebugBubble nodeId={id} position="top" />
      <NodeInputHandles nodeId={id} color={color} />

      <div className="literal-node-content">
        <span className="literal-type-icon" style={{ color }}>
          <Icon name={typeIcon} size={14} />
        </span>
        <span className="literal-value">{formatValue(data.value)}</span>
      </div>
    </div>
  );
});
