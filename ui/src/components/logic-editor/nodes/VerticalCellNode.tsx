import { memo, useCallback } from 'react';
import { useReactFlow } from '@xyflow/react';
import type { VerticalCellNodeData, LogicNode } from '../types';
import { CATEGORY_COLORS } from '../types';
import { useDebugClassName, useNodeCollapse } from '../hooks';
import { NodeInputHandles, CollapseToggleButton, NodeDebugBubble } from './shared';
import { CellRow } from './CellRow';
import { Icon } from '../utils/icons';
import { ExpressionSyntax } from '../utils/ExpressionSyntax';

interface VerticalCellNodeProps {
  id: string;
  data: VerticalCellNodeData;
  selected?: boolean;
}

export const VerticalCellNode = memo(function VerticalCellNode({
  id,
  data,
  selected,
}: VerticalCellNodeProps) {
  const { setNodes } = useReactFlow();
  const color = CATEGORY_COLORS[data.category];
  const collapsedIndices = data.collapsedCellIndices || [];
  const debugClassName = useDebugClassName(id);
  const toggleNodeCollapse = useNodeCollapse(id);

  // Node is collapsible if it has more than 1 arg (any type)
  const canCollapse = data.cells.length > 1;
  const isCollapsed = canCollapse ? (data.collapsed ?? false) : false;

  // Toggle collapse for individual cells
  const toggleCellCollapse = useCallback(
    (cellIndex: number, e: React.MouseEvent) => {
      e.stopPropagation();
      setNodes((nodes) =>
        nodes.map((node) => {
          if (node.id === id) {
            const nodeData = node.data as VerticalCellNodeData;
            const currentCollapsed = nodeData.collapsedCellIndices || [];
            const isCellCollapsed = currentCollapsed.includes(cellIndex);

            const newCollapsed = isCellCollapsed
              ? currentCollapsed.filter((i) => i !== cellIndex)
              : [...currentCollapsed, cellIndex];

            return {
              ...node,
              data: {
                ...nodeData,
                collapsedCellIndices: newCollapsed,
              },
            } as LogicNode;
          }
          return node;
        })
      );
    },
    [id, setNodes]
  );

  return (
    <div
      className={`vertical-cell-node ${selected ? 'selected' : ''} ${isCollapsed ? 'collapsed' : ''} ${debugClassName}`}
      style={{
        borderColor: color,
        backgroundColor: `${color}10`,
      }}
    >
      <NodeDebugBubble nodeId={id} position="top" />
      <NodeInputHandles nodeId={id} color={color} />

      {/* Header with icon, operator, and collapse toggle */}
      <div className="vertical-cell-header" style={{ backgroundColor: color }}>
        <span className="vertical-cell-icon">
          <Icon name={data.icon} size={14} />
        </span>
        <span className="vertical-cell-label">{data.label}</span>
        {canCollapse && (
          <CollapseToggleButton isCollapsed={isCollapsed} onClick={toggleNodeCollapse} />
        )}
      </div>

      {/* Body: either expression text (collapsed) or cell list (expanded) */}
      {isCollapsed ? (
        <div className="vertical-cell-body collapsed-body">
          <div className="expression-text">
            <ExpressionSyntax text={data.expressionText || '...'} />
          </div>
        </div>
      ) : (
        <div className="vertical-cell-body">
          {data.cells.map((cell) => (
            <CellRow
              key={cell.index}
              cell={cell}
              collapsedIndices={collapsedIndices}
              color={color}
              onToggleCollapse={toggleCellCollapse}
            />
          ))}
        </div>
      )}
    </div>
  );
});
