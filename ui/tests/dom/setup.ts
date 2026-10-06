/**
 * Shared setup for the jsdom component tests (`*.dom.test.tsx`).
 *
 * jsdom lacks the layout APIs React Flow and the editor touch, so they get
 * minimal stand-ins: `ResizeObserver`, `DOMMatrixReadOnly`, `matchMedia`
 * and element sizes. Nothing here measures real layout; the tests check
 * structure and behaviour, not pixels.
 */
import { afterEach } from 'vitest';
import { act, cleanup, configure } from '@testing-library/react';

// The full suite runs files in parallel; give async queries room under load.
configure({ asyncUtilTimeout: 5000 });

class ResizeObserverStub {
  observe() {}
  unobserve() {}
  disconnect() {}
}

class DOMMatrixReadOnlyStub {
  m22: number;
  constructor(transform?: string) {
    const scale = transform?.match(/scale\(([0-9.]+)\)/)?.[1];
    this.m22 = scale !== undefined ? Number(scale) : 1;
  }
}

const g = globalThis as Record<string, unknown>;
g.ResizeObserver ??= ResizeObserverStub;
g.DOMMatrixReadOnly ??= DOMMatrixReadOnlyStub;

if (typeof window !== 'undefined') {
  window.matchMedia ??= ((query: string) => ({
    matches: false,
    media: query,
    onchange: null,
    addEventListener() {},
    removeEventListener() {},
    addListener() {},
    removeListener() {},
    dispatchEvent: () => false,
  })) as unknown as typeof window.matchMedia;

  // React Flow reads these to size nodes and the pane.
  Object.defineProperties(HTMLElement.prototype, {
    offsetHeight: { configurable: true, get: () => 100 },
    offsetWidth: { configurable: true, get: () => 200 },
  });
  // jsdom implements neither.
  Element.prototype.scrollIntoView ??= function scrollIntoView() {};
  if (typeof SVGElement !== 'undefined') {
    (SVGElement.prototype as unknown as { getBBox: () => DOMRect }).getBBox ??= () =>
      ({ x: 0, y: 0, width: 0, height: 0 }) as DOMRect;
  }
}

afterEach(() => {
  cleanup();
});

/**
 * Let pending passive effects run. A node can be in the DOM before the
 * effects that subscribe to its selection (React Flow's
 * useOnSelectionChange) have run; a click in that gap, which no user can
 * make, would be lost.
 */
export async function flushEffects(): Promise<void> {
  await act(async () => {
    await new Promise((resolve) => setTimeout(resolve, 0));
  });
}
