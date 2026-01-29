import { memo, useCallback, useMemo, useState, useRef } from 'react';
import { Plus } from 'lucide-react';
import type { VerticalCellNodeData } from '../types';
import { CATEGORY_COLORS } from '../types';
import { useDebugClassName, useNodeCollapse } from '../hooks';
import { useEditorContext } from '../context/editor';
import { getOperator } from '../config/operators';
import { NodeInputHandles, CollapseToggleButton, NodeDebugBubble } from './shared';
import { CellRow } from './CellRow';
import { Icon } from '../utils/icons';
import { ExpressionSyntax } from '../utils/ExpressionSyntax';
import { AddArgumentMenu, type AddArgumentNodeType } from '../context-menu';

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
  const color = CATEGORY_COLORS[data.category];
  const debugClassName = useDebugClassName(id);
  const toggleNodeCollapse = useNodeCollapse(id);
  const { isEditMode, addArgumentToNode } = useEditorContext();

  // State for the add argument menu
  const [menuPosition, setMenuPosition] = useState<{ x: number; y: number } | null>(null);
  const addButtonRef = useRef<HTMLButtonElement>(null);

  // Get operator config for arity information
  const opConfig = getOperator(data.operator);

  // Calculate max args based on arity type
  const maxArgs = useMemo(() => {
    if (!opConfig) return 0;
    const { arity } = opConfig;

    // Fixed arity types don't allow adding
    if (arity.type === 'nullary' || arity.type === 'unary' ||
        arity.type === 'binary' || arity.type === 'ternary') {
      return arity.type === 'nullary' ? 0 :
             arity.type === 'unary' ? 1 :
             arity.type === 'binary' ? 2 : 3;
    }

    // Variable arity - use max if defined, otherwise unlimited
    if (arity.type === 'nary' || arity.type === 'variadic' || arity.type === 'chainable') {
      return arity.max ?? Infinity;
    }

    // Range/special - use max if defined
    return arity.max ?? Infinity;
  }, [opConfig]);

  const canAddArg = isEditMode && data.cells.length < maxArgs;

  // Calculate remaining slots for the add button label
  const remainingSlots = useMemo(() => {
    if (!canAddArg || maxArgs === Infinity) return null;
    return maxArgs - data.cells.length;
  }, [canAddArg, maxArgs, data.cells.length]);

  // Handle opening the add argument menu
  const handleAddArgumentClick = useCallback(
    (e: React.MouseEvent) => {
      e.stopPropagation();
      // Get button's actual screen position
      if (addButtonRef.current) {
        const rect = addButtonRef.current.getBoundingClientRect();
        // Position menu below and to the right of the button
        setMenuPosition({ x: rect.right, y: rect.bottom });
      }
    },
    []
  );

  // Handle menu close
  const handleMenuClose = useCallback(() => {
    setMenuPosition(null);
  }, []);

  // Handle menu selection
  const handleMenuSelect = useCallback(
    (type: AddArgumentNodeType, operatorName?: string) => {
      addArgumentToNode(id, type, operatorName);
      setMenuPosition(null);
    },
    [id, addArgumentToNode]
  );

  // Node is collapsible if it has more than 1 arg (any type)
  const canCollapse = data.cells.length > 1;
  const isCollapsed = canCollapse ? (data.collapsed ?? false) : false;

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
              color={color}
            />
          ))}
          {/* Add Row button for variable arity operators */}
          {canAddArg && (
            <button
              ref={addButtonRef}
              type="button"
              className="add-arg-button add-arg-button--vertical"
              onClick={handleAddArgumentClick}
              title={remainingSlots ? `Add row (${remainingSlots} more available)` : 'Add row'}
            >
              <Plus size={12} />
              <span className="add-arg-button-label">
                {remainingSlots ? `Add row (${remainingSlots} more)` : 'Add row'}
              </span>
            </button>
          )}
        </div>
      )}

      {/* Add argument menu */}
      {menuPosition && (
        <AddArgumentMenu
          x={menuPosition.x}
          y={menuPosition.y}
          onClose={handleMenuClose}
          onSelect={handleMenuSelect}
          operatorCategory={data.category}
        />
      )}
    </div>
  );
});
