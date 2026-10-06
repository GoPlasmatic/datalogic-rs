// @vitest-environment jsdom
/**
 * A controlled editor reports each edit through `onChange`; the host feeds
 * the value back, and the editor re-converts it. That round trip used to
 * rebuild every node with fresh ids, which remounted the canvas and closed
 * the properties panel 300 ms after each edit.
 */
import { useState } from 'react';
import { describe, expect, it, vi } from 'vitest';
import { act, fireEvent, render, waitFor } from '@testing-library/react';
import { flushEffects } from './setup';
import { DataLogicEditor } from '../../src/lib';
import type { JsonLogicValue } from '../../src/lib';

function Controlled({ initial, onValue }: { initial: JsonLogicValue; onValue?: (v: JsonLogicValue | null) => void }) {
  const [value, setValue] = useState<JsonLogicValue | null>(initial);
  return (
    <DataLogicEditor
      value={value}
      onChange={(next) => {
        onValue?.(next);
        setValue(next);
      }}
      editable
    />
  );
}

/** Let the panel's 200 ms apply and the editor's 300 ms onChange debounce run. */
async function settle(ms = 800) {
  await act(async () => {
    await new Promise((resolve) => setTimeout(resolve, ms));
  });
}

function nodeWithText(container: HTMLElement, text: string): HTMLElement {
  const nodes = Array.from(container.querySelectorAll<HTMLElement>('.react-flow__node'));
  const node = nodes.find((n) => n.textContent?.trim() === text);
  if (!node) throw new Error(`no node "${text}" among: ${nodes.map((n) => n.textContent).join(' | ')}`);
  return node;
}

describe('editing a controlled editor', () => {
  it('keeps the selection and the properties panel across the onChange round trip', async () => {
    const onValue = vi.fn();
    const { container } = render(
      <Controlled initial={{ if: [{ var: 'x' }, 'yes', 'no'] }} onValue={onValue} />,
    );
    await waitFor(() => expect(container.querySelectorAll('.react-flow__node').length).toBeGreaterThan(1));

    const yes = nodeWithText(container, 'str"yes"');
    const selectedId = yes.getAttribute('data-id');
    await flushEffects();
    fireEvent.click(yes);
    await waitFor(() => expect(container.querySelector('.properties-panel')).not.toBeNull());

    const input = container.querySelector<HTMLInputElement>('.properties-panel input[type="text"], .properties-panel textarea');
    expect(input).not.toBeNull();
    fireEvent.change(input!, { target: { value: 'maybe' } });
    await settle();

    expect(onValue).toHaveBeenLastCalledWith({ if: [{ var: 'x' }, 'maybe', 'no'] });
    expect(container.querySelector('.properties-panel')).not.toBeNull();
    expect(container.querySelector(`.react-flow__node[data-id="${selectedId}"]`)?.textContent).toContain('maybe');
  });

  it('gives a re-converted rule the same node ids', async () => {
    const { container, rerender } = render(<DataLogicEditor value={{ '>': [{ var: 'a' }, 1] }} editable onChange={() => {}} />);
    await waitFor(() => expect(container.querySelectorAll('.react-flow__node').length).toBeGreaterThan(0));
    const before = Array.from(container.querySelectorAll('.react-flow__node')).map((n) => n.getAttribute('data-id'));

    rerender(<DataLogicEditor value={{ '>': [{ var: 'a' }, 2] }} editable onChange={() => {}} />);
    await waitFor(() => expect(literalNodeText(container)).toContain('2'));
    const after = Array.from(container.querySelectorAll('.react-flow__node')).map((n) => n.getAttribute('data-id'));
    expect(after).toEqual(before);
  });

  it('still shows an external change that keeps the node count', async () => {
    const { container, rerender } = render(<DataLogicEditor value={{ '>': [{ var: 'a' }, 1] }} editable onChange={() => {}} />);
    await waitFor(() => expect(container.querySelectorAll('.react-flow__node').length).toBeGreaterThan(0));
    rerender(<DataLogicEditor value={{ '<': [{ var: 'b' }, 1] }} editable onChange={() => {}} />);
    await waitFor(() => expect(container.textContent).toContain('b'));
  });
});

function literalNodeText(container: HTMLElement): string {
  return Array.from(container.querySelectorAll('.react-flow__node')).map((n) => n.textContent).join(' ');
}
