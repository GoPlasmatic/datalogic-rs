/**
 * CanvasContextMenu Component
 *
 * Context menu for canvas (pane) operations:
 * - Add Variable
 * - Add Literal
 * - Add Operator (submenu by category, shared with every other picker)
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
import { buildOperatorSubmenu } from '../utils/menu-builder';
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

  // Operator submenu grouped by category (single source of truth)
  const operatorSubmenu = useMemo<MenuItemConfig[]>(
    () => buildOperatorSubmenu((opName) => createNode('operator', opName)),
    [createNode]
  );

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
      shortcut: '⌘V',
      disabled: !canPaste,
      onClick: () => pasteNode(),
    });

    // Select All
    items.push({
      id: 'select-all',
      label: 'Select All',
      icon: <MousePointer2 size={14} />,
      shortcut: '⌘A',
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
