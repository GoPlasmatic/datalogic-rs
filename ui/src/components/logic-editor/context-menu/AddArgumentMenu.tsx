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
import { getOperatorsGroupedByCategory } from '../config/operators';
import type { OperatorCategory } from '../config/operators.types';

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

function capitalizeFirst(str: string): string {
  return str.charAt(0).toUpperCase() + str.slice(1);
}

export const AddArgumentMenu = memo(function AddArgumentMenu({
  x,
  y,
  onClose,
  onSelect,
}: AddArgumentMenuProps) {
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
          onClick: () => onSelect('operator', op.name),
        })),
      });
    }

    return items;
  }, [onSelect]);

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
