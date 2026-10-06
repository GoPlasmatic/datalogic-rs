// @vitest-environment jsdom
/**
 * Smoke tests: render DataLogicEditor in each prop-driven mode.
 */
import { describe, expect, it } from 'vitest';
import { render, screen, waitFor } from '@testing-library/react';
import './setup';
import { DataLogicEditor } from '../../src/lib';
import type { JsonLogicValue } from '../../src/lib';

const RULE: JsonLogicValue = { '>': [{ var: 'age' }, 18] };

describe('DataLogicEditor', () => {
  it('renders a read-only diagram', async () => {
    const { container } = render(<DataLogicEditor value={RULE} />);
    await waitFor(() => expect(container.querySelectorAll('.react-flow__node').length).toBeGreaterThan(0));
    expect(container.querySelector('.logic-editor')).not.toBeNull();
    expect(container.querySelector('.properties-panel')).toBeNull();
  });

  it('renders the empty state for a null value', () => {
    render(<DataLogicEditor value={null} />);
    expect(screen.getByText('No expression')).toBeTruthy();
  });

  it('shows the debugger toolbar when data is provided', async () => {
    const { container } = render(<DataLogicEditor value={RULE} data={{ age: 30 }} />);
    await waitFor(() => expect(container.querySelector('.dl-debugger-controls--inline')).not.toBeNull());
  });

  it('renders the editing chrome when editable', async () => {
    const { container } = render(<DataLogicEditor value={RULE} editable onChange={() => {}} />);
    await waitFor(() => expect(container.querySelectorAll('.react-flow__node').length).toBeGreaterThan(0));
    expect(screen.getByRole('button', { name: /insert/i })).toBeTruthy();
  });
});
