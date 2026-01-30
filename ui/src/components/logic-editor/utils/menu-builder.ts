/**
 * Menu Builder Utility
 *
 * Provides shared logic for building operator submenus.
 * Used by AddArgumentMenu and NodeContextMenu to ensure
 * consistent operator grouping and presentation.
 */

import type { MenuItemConfig } from '../context-menu/ContextMenu';
import { getOperatorsGroupedByCategory } from '../config/operators';
import type { OperatorCategory } from '../config/operators.types';

/**
 * Capitalize the first letter of a string.
 */
export function capitalizeFirst(str: string): string {
  return str.charAt(0).toUpperCase() + str.slice(1);
}

/**
 * Standard category order for operator menus.
 * This ensures consistent ordering across all menus.
 */
const OPERATOR_CATEGORY_ORDER: OperatorCategory[] = [
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

/**
 * Options for building operator submenus.
 */
export interface OperatorMenuOptions {
  /** Categories to exclude from the menu */
  excludeCategories?: OperatorCategory[];
  /** Maximum number of operators per category (default: 10) */
  maxPerCategory?: number;
}

/**
 * Build operator submenu items grouped by category.
 *
 * This creates a consistent menu structure for selecting operators,
 * with operators grouped into category submenus.
 *
 * @param onSelect - Callback when an operator is selected
 * @param options - Optional configuration
 * @returns Array of menu items for use in ContextMenu
 */
export function buildOperatorSubmenu(
  onSelect: (operatorName: string) => void,
  options?: OperatorMenuOptions
): MenuItemConfig[] {
  const {
    excludeCategories = [],
    maxPerCategory = 10,
  } = options ?? {};

  const grouped = getOperatorsGroupedByCategory();
  const items: MenuItemConfig[] = [];

  for (const category of OPERATOR_CATEGORY_ORDER) {
    // Skip excluded categories
    if (excludeCategories.includes(category)) continue;

    const operators = grouped.get(category);
    if (!operators || operators.length === 0) continue;

    items.push({
      id: `category-${category}`,
      label: capitalizeFirst(category),
      submenu: operators.slice(0, maxPerCategory).map((op) => ({
        id: `op-${op.name}`,
        label: op.label || op.name,
        onClick: () => onSelect(op.name),
      })),
    });
  }

  return items;
}
