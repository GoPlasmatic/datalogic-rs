// @vitest-environment jsdom
/**
 * The properties panel's field ids are unique per editor, so labels bind
 * to their own editor's inputs when a page shows several editors.
 */
import { describe, expect, it } from 'vitest';
import { fireEvent, render, waitFor } from '@testing-library/react';
import './setup';
import { DataLogicEditor } from '../../src/lib';

async function openPanel(root: HTMLElement) {
  await waitFor(() => expect(root.querySelector('.react-flow__node')).not.toBeNull());
  fireEvent.click(root.querySelector('.react-flow__node')!);
  await waitFor(() => expect(root.querySelector('.panel-field label[for]')).not.toBeNull());
}

describe('properties panel', () => {
  it('binds every label to an input in its own editor', async () => {
    const { container } = render(
      <>
        <DataLogicEditor value={42} editable onChange={() => {}} />
        <DataLogicEditor value={7} editable onChange={() => {}} />
      </>,
    );
    const roots = Array.from(container.querySelectorAll<HTMLElement>('.logic-editor'));
    expect(roots).toHaveLength(2);
    for (const root of roots) await openPanel(root);

    const ids = new Set<string>();
    for (const root of roots) {
      for (const label of Array.from(root.querySelectorAll('.panel-field label[for]'))) {
        const id = label.getAttribute('for')!;
        expect(ids.has(id)).toBe(false);
        ids.add(id);
        const control = document.getElementById(id);
        expect(control).not.toBeNull();
        expect(root.contains(control)).toBe(true);
      }
    }
    expect(ids.size).toBeGreaterThan(1);
  });
});
