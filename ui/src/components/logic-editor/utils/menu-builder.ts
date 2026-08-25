/**
 * Menu Builder Utility
 *
 * Single source of truth for the "pick an operator" submenu used by the
 * canvas context menu, the node context menu (Add Argument / Wrap) and the
 * toolbar Insert menu, so every entry point lists the same operators in
 * the same order.
 */

import type { MenuItemConfig } from '../context-menu/ContextMenu';
import { getOperatorsGroupedByCategory } from '../config/operators';
import { categories } from '../config/categories';
import type { OperatorCategory } from '../config/operators.types';

/**
 * Capitalize the first letter of a string.
 */
export function capitalizeFirst(str: string): string {
  return str.charAt(0).toUpperCase() + str.slice(1);
}

/**
 * Category order for operator menus, derived from the category registry
 * (declaration order in `config/categories.ts`). Any category that only
 * exists in the operator registry is appended so nothing is ever hidden.
 */
export function getOperatorCategoryOrder(): OperatorCategory[] {
  const known = Object.keys(categories) as OperatorCategory[];
  const seen = new Set<OperatorCategory>(known);
  const extra: OperatorCategory[] = [];
  for (const category of getOperatorsGroupedByCategory().keys()) {
    if (!seen.has(category)) {
      seen.add(category);
      extra.push(category);
    }
  }
  return [...known, ...extra];
}

/**
 * Options for building operator submenus.
 */
export interface OperatorMenuOptions {
  /** Categories to exclude from the menu */
  excludeCategories?: OperatorCategory[];
  /** Maximum number of operators per category (default: unlimited) */
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
    maxPerCategory = Infinity,
  } = options ?? {};

  const grouped = getOperatorsGroupedByCategory();
  const items: MenuItemConfig[] = [];

  for (const category of getOperatorCategoryOrder()) {
    // Skip excluded categories
    if (excludeCategories.includes(category)) continue;

    const operators = grouped.get(category);
    if (!operators || operators.length === 0) continue;

    const visible = Number.isFinite(maxPerCategory)
      ? operators.slice(0, maxPerCategory)
      : operators;

    items.push({
      id: `category-${category}`,
      label: categories[category]?.label ?? capitalizeFirst(category),
      submenu: visible.map((op) => ({
        id: `op-${op.name}`,
        label: op.label || op.name,
        onClick: () => onSelect(op.name),
      })),
    });
  }

  return items;
}
