/**
 * NodeContextMenu Component
 *
 * Context menu for node operations:
 * - Edit Properties (focus properties panel)
 * - Add/Remove Argument (for n-ary operators)
 * - Wrap in Operator submenu
 * - Duplicate, Copy, Paste as Child
 * - Collapse/Expand
 * - Select Children
 * - Delete
 */

import { memo } from 'react';
import { createPortal } from 'react-dom';
import { ContextMenu } from './ContextMenu';
import type { LogicNode } from '../types';
import { useContextMenuItems } from './useContextMenuItems';

export interface NodeContextMenuProps {
  /** X position (screen coordinates) */
  x: number;
  /** Y position (screen coordinates) */
  y: number;
  /** The node that was right-clicked */
  node: LogicNode;
  /** Called when menu should close */
  onClose: () => void;
  /** Called when "Edit Properties" is selected */
  onEditProperties?: () => void;
}

export const NodeContextMenu = memo(function NodeContextMenu({
  x,
  y,
  node,
  onClose,
  onEditProperties,
}: NodeContextMenuProps) {
  const menuItems = useContextMenuItems({ node, onEditProperties, onClose });

  // Use a portal to render outside of ReactFlow's transformed container
  return createPortal(
    <ContextMenu x={x} y={y} items={menuItems} onClose={onClose} />,
    document.body
  );
});
