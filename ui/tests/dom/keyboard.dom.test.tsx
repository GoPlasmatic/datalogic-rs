// @vitest-environment jsdom
/**
 * Keyboard shortcuts belong to the editor that has focus. They used to be
 * bound on `window`, so every editor on a page reacted to every key, and
 * the host page lost Space, the arrows, Cmd/Ctrl+A and friends.
 */
import { describe, expect, it } from 'vitest';
import { fireEvent, render, waitFor } from '@testing-library/react';
import './setup';
import { DataLogicEditor } from '../../src/lib';
import type { JsonLogicValue } from '../../src/lib';

const RULE: JsonLogicValue = { '+': [1, { var: 'x' }] };

function stepOf(root: Element): string | null | undefined {
  return root.querySelector('.dl-step-current')?.textContent;
}

async function renderTwoDebuggers() {
  const { container } = render(
    <>
      <DataLogicEditor value={RULE} data={{ x: 2 }} />
      <DataLogicEditor value={RULE} data={{ x: 3 }} />
    </>,
  );
  await waitFor(() => expect(container.querySelectorAll('.dl-step-current')).toHaveLength(2));
  const [a, b] = Array.from(container.querySelectorAll<HTMLElement>('.logic-editor'));
  return { a, b };
}

describe('debugger shortcuts', () => {
  it('drive only the focused editor', async () => {
    const { a, b } = await renderTwoDebuggers();
    expect(stepOf(a)).toBe('0');

    a.focus();
    fireEvent.keyDown(a, { key: 'ArrowRight' });

    expect(stepOf(a)).toBe('1');
    expect(stepOf(b)).toBe('0');
  });

  it('leave keys pressed outside every editor to the page', async () => {
    const { a, b } = await renderTwoDebuggers();
    const event = new KeyboardEvent('keydown', { key: 'ArrowRight', bubbles: true, cancelable: true });
    document.body.dispatchEvent(event);

    expect(event.defaultPrevented).toBe(false);
    expect(stepOf(a)).toBe('0');
    expect(stepOf(b)).toBe('0');
  });

  it('leave Space to a focused button', async () => {
    const { a } = await renderTwoDebuggers();
    const button = a.querySelector<HTMLButtonElement>('.dl-debugger-btn')!;
    const event = new KeyboardEvent('keydown', { key: ' ', bubbles: true, cancelable: true });
    button.dispatchEvent(event);
    expect(event.defaultPrevented).toBe(false);
  });

  it('make the editor root focusable without adding a tab stop', async () => {
    const { a } = await renderTwoDebuggers();
    expect(a.getAttribute('tabindex')).toBe('-1');
  });
});

describe('editing shortcuts', () => {
  it('ignore keys pressed outside the editor', async () => {
    const { container } = render(<DataLogicEditor value={RULE} editable onChange={() => {}} />);
    await waitFor(() => expect(container.querySelectorAll('.react-flow__node').length).toBeGreaterThan(0));

    const outside = new KeyboardEvent('keydown', { key: 'a', ctrlKey: true, metaKey: true, bubbles: true, cancelable: true });
    document.body.dispatchEvent(outside);
    expect(outside.defaultPrevented).toBe(false);

    const root = container.querySelector<HTMLElement>('.logic-editor')!;
    const inside = new KeyboardEvent('keydown', { key: 'a', ctrlKey: true, metaKey: true, bubbles: true, cancelable: true });
    root.dispatchEvent(inside);
    expect(inside.defaultPrevented).toBe(true);
  });
});
