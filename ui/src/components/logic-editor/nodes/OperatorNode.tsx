import { memo } from 'react';
import { Handle, Position } from '@xyflow/react';
import type { OperatorNodeData } from '../types';
import { CATEGORY_COLORS } from '../types';
import { useDebugClassName, useNodeCollapse } from '../hooks';
import { NodeInputHandles, CollapseToggleButton, NodeDebugBubble } from './shared';
import { ExpressionSyntax } from '../utils/ExpressionSyntax';

interface OperatorNodeProps {
  id: string;
  data: OperatorNodeData;
  selected?: boolean;
}

export const OperatorNode = memo(function OperatorNode({
  id,
  data,
  selected,
}: OperatorNodeProps) {
  const color = CATEGORY_COLORS[data.category];
  const debugClassName = useDebugClassName(id);
  const toggleCollapse = useNodeCollapse(id);

  // Check if this is an inline-only node (unary operator with simple arg)
  const isInlineOnly = !!data.inlineDisplay;

  // Any operator expression is collapsible (shows expression text when collapsed)
  // But inline-only nodes cannot collapse
  const canCollapse = !isInlineOnly && data.childIds.length >= 1;
  const isCollapsed = canCollapse ? (data.collapsed ?? false) : false;

  return (
    <div
      className={`operator-node ${selected ? 'selected' : ''} ${isCollapsed ? 'collapsed' : ''} ${debugClassName}`}
      style={{
        borderColor: color,
        backgroundColor: `${color}10`,
      }}
    >
      <NodeDebugBubble nodeId={id} position="top" />
      <NodeInputHandles nodeId={id} color={color} />

      <div className="operator-node-header" style={{ backgroundColor: color }}>
        <div className="operator-header-content">
          <span className="operator-label">{data.label}</span>
          {canCollapse && (
            <CollapseToggleButton isCollapsed={isCollapsed} onClick={toggleCollapse} />
          )}
        </div>
      </div>

      <div className="operator-node-body">
        {isInlineOnly ? (
          // Inline-only display for unary operators with simple args
          <div className="expression-text inline-expression">
            <ExpressionSyntax text={data.inlineDisplay || ''} />
          </div>
        ) : isCollapsed ? (
          <div className="expression-text">
            <ExpressionSyntax text={data.expressionText || '...'} />
          </div>
        ) : (
          <>
            <span className="operator-category">{data.category}</span>
            <span className="operator-children-count">
              {data.childIds.length} arg{data.childIds.length !== 1 ? 's' : ''}
            </span>
          </>
        )}
      </div>

      {/* Output handles for children - only show when expanded, positioned on right side */}
      {!isCollapsed &&
        data.childIds.map((_, index) => {
          // Calculate vertical position for each handle
          // Header is ~36px, body starts after, distribute handles evenly on right side
          const headerHeight = 36;
          const bodyHeight = 48; // Approximate body height
          const handleTop = headerHeight + ((index + 1) / (data.childIds.length + 1)) * bodyHeight;

          return (
            <Handle
              key={index}
              type="source"
              position={Position.Right}
              id={`arg-${index}`}
              style={{
                background: color,
                top: `${handleTop}px`,
              }}
            />
          );
        })}
    </div>
  );
});
