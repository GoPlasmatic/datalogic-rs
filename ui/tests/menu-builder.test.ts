import { describe, expect, it } from 'vitest';
import { buildOperatorSubmenu, getOperatorCategoryOrder } from '../src/components/logic-editor/utils/menu-builder';
import { operators, getOperatorsGroupedByCategory } from '../src/components/logic-editor/config/operators';
import { categories } from '../src/components/logic-editor/config/categories';

function collectOperatorIds(items: ReturnType<typeof buildOperatorSubmenu>): string[] {
  const ids: string[] = [];
  for (const category of items) {
    for (const entry of category.submenu ?? []) {
      ids.push(entry.id.replace(/^op-/, ''));
    }
  }
  return ids;
}

describe('buildOperatorSubmenu', () => {
  it('lists every registered operator exactly once (no per-category cap)', () => {
    const selected: string[] = [];
    const items = buildOperatorSubmenu((name) => selected.push(name));
    const listed = collectOperatorIds(items);
    expect(listed.sort()).toEqual(Object.keys(operators).sort());
    expect(new Set(listed).size).toBe(listed.length);
  });

  it('includes every category that has operators, in registry order', () => {
    const items = buildOperatorSubmenu(() => {});
    const grouped = getOperatorsGroupedByCategory();
    const expected = getOperatorCategoryOrder().filter((c) => (grouped.get(c)?.length ?? 0) > 0);
    expect(items.map((i) => i.id.replace(/^category-/, ''))).toEqual(expected);
    // The object category (keys/values/entries) used to be missing from the canvas menu.
    expect(items.map((i) => i.id)).toContain('category-object');
  });

  it('derives the category order from categories.ts', () => {
    const order = getOperatorCategoryOrder();
    for (const known of Object.keys(categories)) {
      expect(order).toContain(known);
    }
    // Nothing from the operator registry is dropped even if it lacks category metadata.
    for (const category of getOperatorsGroupedByCategory().keys()) {
      expect(order).toContain(category);
    }
  });

  it('wires onSelect to the operator name', () => {
    const selected: string[] = [];
    const items = buildOperatorSubmenu((name) => selected.push(name));
    const arrayMenu = items.find((i) => i.id === 'category-array');
    const distinct = arrayMenu?.submenu?.find((i) => i.id === 'op-distinct');
    expect(distinct).toBeDefined();
    distinct?.onClick?.();
    expect(selected).toEqual(['distinct']);
  });

  it('still honours an explicit maxPerCategory and excludeCategories', () => {
    const items = buildOperatorSubmenu(() => {}, { maxPerCategory: 2, excludeCategories: ['error'] });
    expect(items.every((i) => (i.submenu?.length ?? 0) <= 2)).toBe(true);
    expect(items.map((i) => i.id)).not.toContain('category-error');
  });
});
