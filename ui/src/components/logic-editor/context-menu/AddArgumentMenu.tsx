/**
 * AddArgumentMenu Component
 *
 * Dropdown menu for adding a new argument to an operator node.
 * Shows node type options: Literal, Variable, Operator (with submenu)
 *
 * Uses a Portal to render the menu outside of ReactFlow's transformed container,
 * ensuring correct positioning regardless of zoom/pan state.
 */

import { memo, useMemo } from 'react';
import { createPortal } from 'react-dom';
import { Hash, Variable, Calculator } from 'lucide-react';
import { ContextMenu, type MenuItemConfig } from './ContextMenu';
import { buildOperatorSubmenu } from '../utils/menu-builder';

export type AddArgumentNodeType = 'literal' | 'variable' | 'operator';

export interface AddArgumentMenuProps {
  /** X position (screen coordinates) */
  x: number;
  /** Y position (screen coordinates) */
  y: number;
  /** Called when menu should close */
  onClose: () => void;
  /** Called when a node type is selected */
  onSelect: (type: AddArgumentNodeType, operatorName?: string) => void;
  /** Operator category hint for default values (unused, reserved for future) */
  operatorCategory?: string;
}

export const AddArgumentMenu = memo(function AddArgumentMenu({
  x,
  y,
  onClose,
  onSelect,
}: AddArgumentMenuProps) {
  // Build operator submenu using the shared utility
  const operatorSubmenu = useMemo<MenuItemConfig[]>(
    () => buildOperatorSubmenu((opName) => onSelect('operator', opName)),
    [onSelect]
  );

  // Build menu items
  const menuItems = useMemo<MenuItemConfig[]>(() => {
    const items: MenuItemConfig[] = [];

    // Literal value
    items.push({
      id: 'add-literal',
      label: 'Literal Value',
      icon: <Hash size={14} />,
      onClick: () => onSelect('literal'),
    });

    // Variable reference
    items.push({
      id: 'add-variable',
      label: 'Variable',
      icon: <Variable size={14} />,
      onClick: () => onSelect('variable'),
    });

    items.push({ id: 'divider' } as MenuItemConfig);

    // Operator submenu
    items.push({
      id: 'add-operator',
      label: 'Operator',
      icon: <Calculator size={14} />,
      submenu: operatorSubmenu,
    });

    return items;
  }, [onSelect, operatorSubmenu]);

  // Use a portal to render outside of ReactFlow's transformed container
  return createPortal(
    <ContextMenu x={x} y={y} items={menuItems} onClose={onClose} />,
    document.body
  );
});
