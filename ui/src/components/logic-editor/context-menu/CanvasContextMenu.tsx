/**
 * CanvasContextMenu Component
 *
 * Context menu for canvas (pane) operations:
 * - Add Variable
 * - Add Literal
 * - Add Operator (submenu by category)
 * - Add Condition
 * - Paste
 * - Select All
 * - Fit View
 * - Reset Zoom
 */

import { memo, useMemo } from 'react';
import {
  Variable,
  Hash,
  Calculator,
  GitBranch,
  Clipboard,
  MousePointer2,
  Maximize,
  ZoomIn,
} from 'lucide-react';
import { useReactFlow } from '@xyflow/react';
import { ContextMenu, type MenuItemConfig } from './ContextMenu';
import { useEditorContext } from '../context/editor';
import { getOperatorsGroupedByCategory } from '../config/operators';
import type { OperatorCategory } from '../config/operators.types';
import { REACT_FLOW_OPTIONS } from '../constants/layout';

export interface CanvasContextMenuProps {
  /** X position (screen coordinates) */
  x: number;
  /** Y position (screen coordinates) */
  y: number;
  /** Called when menu should close */
  onClose: () => void;
}

export const CanvasContextMenu = memo(function CanvasContextMenu({
  x,
  y,
  onClose,
}: CanvasContextMenuProps) {
  const {
    createNode,
    pasteNode,
    canPaste,
    selectAllNodes,
    hasNodes,
  } = useEditorContext();

  const { fitView, zoomTo } = useReactFlow();

  // Build operator submenu grouped by category
  const operatorSubmenu = useMemo<MenuItemConfig[]>(() => {
    const grouped = getOperatorsGroupedByCategory();
    const items: MenuItemConfig[] = [];

    // Priority order for categories
    const categoryOrder = [
      'arithmetic',
      'comparison',
      'logical',
      'string',
      'array',
      'control',
      'datetime',
      'validation',
      'variable',
      'utility',
      'error',
    ];

    for (const category of categoryOrder) {
      const operators = grouped.get(category as OperatorCategory);
      if (!operators || operators.length === 0) continue;

      items.push({
        id: `category-${category}`,
        label: capitalizeFirst(category),
        submenu: operators.slice(0, 10).map((op) => ({
          id: `op-${op.name}`,
          label: op.label || op.name,
          onClick: () => createNode('operator', op.name),
        })),
      });
    }

    return items;
  }, [createNode]);

  // Build menu items
  const menuItems = useMemo<MenuItemConfig[]>(() => {
    const items: MenuItemConfig[] = [];

    // Add Variable
    items.push({
      id: 'add-variable',
      label: 'Add Variable',
      icon: <Variable size={14} />,
      onClick: () => createNode('variable'),
    });

    // Add Literal
    items.push({
      id: 'add-literal',
      label: 'Add Literal',
      icon: <Hash size={14} />,
      onClick: () => createNode('literal'),
    });

    // Add Operator submenu
    items.push({
      id: 'add-operator',
      label: 'Add Operator',
      icon: <Calculator size={14} />,
      submenu: operatorSubmenu,
    });

    // Add Condition
    items.push({
      id: 'add-condition',
      label: 'Add Condition',
      icon: <GitBranch size={14} />,
      onClick: () => createNode('condition'),
    });

    items.push({ id: 'divider' } as MenuItemConfig);

    // Paste
    items.push({
      id: 'paste',
      label: 'Paste',
      icon: <Clipboard size={14} />,
      shortcut: '\u2318V',
      disabled: !canPaste,
      onClick: () => pasteNode(),
    });

    // Select All
    items.push({
      id: 'select-all',
      label: 'Select All',
      icon: <MousePointer2 size={14} />,
      shortcut: '\u2318A',
      disabled: !hasNodes(),
      onClick: () => selectAllNodes(),
    });

    items.push({ id: 'divider' } as MenuItemConfig);

    // Fit View
    items.push({
      id: 'fit-view',
      label: 'Fit View',
      icon: <Maximize size={14} />,
      disabled: !hasNodes(),
      onClick: () =>
        fitView({
          padding: REACT_FLOW_OPTIONS.fitViewPadding,
          maxZoom: REACT_FLOW_OPTIONS.maxZoom,
        }),
    });

    // Reset Zoom
    items.push({
      id: 'reset-zoom',
      label: 'Reset Zoom',
      icon: <ZoomIn size={14} />,
      onClick: () => zoomTo(1),
    });

    return items;
  }, [
    createNode,
    operatorSubmenu,
    canPaste,
    pasteNode,
    hasNodes,
    selectAllNodes,
    fitView,
    zoomTo,
  ]);

  return <ContextMenu x={x} y={y} items={menuItems} onClose={onClose} />;
});

function capitalizeFirst(str: string): string {
  return str.charAt(0).toUpperCase() + str.slice(1);
}
