import { describe, expect, it } from 'vitest';
import { isEditableTarget } from '../keyboard-guard';

/** Minimal stand-in for a DOM element (tests run without a DOM). */
function fakeElement(options: { tag?: string; contentEditable?: boolean; ancestors?: string[] } = {}) {
  const tags = [options.tag ?? 'div', ...(options.ancestors ?? [])].map((t) => t.toLowerCase());
  return {
    tagName: (options.tag ?? 'div').toUpperCase(),
    isContentEditable: options.contentEditable ?? false,
    closest(selector: string) {
      const wanted = selector
        .split(',')
        .map((s) => s.trim().replace(/\[.*$/, ''))
        .filter(Boolean);
      return tags.some((t) => wanted.includes(t)) ? {} : null;
    },
  } as unknown as EventTarget;
}

describe('isEditableTarget', () => {
  it('lets the canvas and plain containers through', () => {
    expect(isEditableTarget(fakeElement({ tag: 'div' }))).toBe(false);
    expect(isEditableTarget(fakeElement({ tag: 'svg' }))).toBe(false);
    expect(isEditableTarget(null)).toBe(false);
    expect(isEditableTarget({} as EventTarget)).toBe(false);
  });

  it('guards inputs, textareas, selects and buttons', () => {
    expect(isEditableTarget(fakeElement({ tag: 'input' }))).toBe(true);
    expect(isEditableTarget(fakeElement({ tag: 'textarea' }))).toBe(true);
    expect(isEditableTarget(fakeElement({ tag: 'select' }))).toBe(true);
    expect(isEditableTarget(fakeElement({ tag: 'button' }))).toBe(true);
  });

  it('guards descendants of a button or select (icon inside a button)', () => {
    expect(isEditableTarget(fakeElement({ tag: 'svg', ancestors: ['button'] }))).toBe(true);
    expect(isEditableTarget(fakeElement({ tag: 'span', ancestors: ['select'] }))).toBe(true);
  });

  it('guards contenteditable regions', () => {
    expect(isEditableTarget(fakeElement({ tag: 'div', contentEditable: true }))).toBe(true);
  });
});
