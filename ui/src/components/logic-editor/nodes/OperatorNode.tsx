import { memo, useCallback, useMemo, useState, useRef } from 'react';
import { Handle, Position } from '@xyflow/react';
import { Plus } from 'lucide-react';
import type { OperatorNodeData } from '../types';
import { CATEGORY_COLORS } from '../types';
import { useDebugClassName, useNodeCollapse } from '../hooks';
import { useEditorContext } from '../context/editor';
import { getOperator } from '../config/operators';
import { NodeInputHandles, CollapseToggleButton, NodeDebugBubble } from './shared';
import { ExpressionSyntax } from '../utils/ExpressionSyntax';
import { AddArgumentMenu, type AddArgumentNodeType } from '../context-menu';

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
  const { isEditMode, addArgumentToNode } = useEditorContext();

  // State for the add argument menu
  const [menuPosition, setMenuPosition] = useState<{ x: number; y: number } | null>(null);
  const addButtonRef = useRef<HTMLButtonElement>(null);

  // Get operator config for arity information
  const opConfig = getOperator(data.operator);

  // Calculate max args based on arity type
  const getMaxArgs = () => {
    if (!opConfig) return 0;
    const { arity } = opConfig;

    // Fixed arity types have specific expected counts
    if (arity.type === 'nullary') return 0;
    if (arity.type === 'unary') return 1;
    if (arity.type === 'binary') return 2;
    if (arity.type === 'ternary') return 3;

    // Variable arity - use max if defined, otherwise unlimited (Infinity)
    if (arity.type === 'nary' || arity.type === 'variadic' || arity.type === 'chainable') {
      return arity.max ?? Infinity;
    }

    // Range/special - use max if defined
    return arity.max ?? Infinity;
  };

  const maxArgs = getMaxArgs();
  const canAddArg = isEditMode && data.childIds.length < maxArgs;

  // Calculate remaining slots for the add button label
  const remainingSlots = useMemo(() => {
    if (!canAddArg || maxArgs === Infinity) return null;
    return maxArgs - data.childIds.length;
  }, [canAddArg, maxArgs, data.childIds.length]);

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
      {!isCollapsed && (
        <>
          {data.childIds.map((_, index) => {
            // Calculate vertical position for each handle
            // Header is ~36px, body starts after, distribute handles evenly on right side
            const headerHeight = 36;
            const bodyHeight = 48; // Approximate body height
            const totalSlots = canAddArg ? data.childIds.length + 1 : data.childIds.length;
            const handleTop = headerHeight + ((index + 1) / (totalSlots + 1)) * bodyHeight;

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

          {/* Add argument button - positioned at bottom of node body */}
          {canAddArg && (
            <button
              ref={addButtonRef}
              type="button"
              className="add-arg-button"
              onClick={handleAddArgumentClick}
              title={remainingSlots ? `Add argument (${remainingSlots} more available)` : 'Add argument'}
            >
              <Plus size={12} />
              <span className="add-arg-button-label">
                {remainingSlots ? `Add (${remainingSlots} more)` : 'Add arg'}
              </span>
            </button>
          )}
        </>
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
