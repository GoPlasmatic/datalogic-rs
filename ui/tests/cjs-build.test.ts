// @vitest-environment jsdom
/**
 * Smoke test for the published CommonJS entry (`dist/index.cjs`, the
 * `require` condition of the package's exports).
 *
 * Rolldown rewrites `import.meta` to `{}` in CJS output, which used to
 * leave wasm-bindgen's loader with an invalid base URL: the engine never
 * started for any `require` consumer. This loads the built file through
 * Node's own `require`, starts the engine through the public
 * `useWasmEvaluator` hook and evaluates one rule.
 *
 * It needs `npm run build:lib` first and is skipped without `dist/`.
 */
import { existsSync } from 'node:fs';
import { createRequire } from 'node:module';
import { resolve } from 'node:path';
import { describe, expect, it } from 'vitest';
import { renderHook, waitFor } from '@testing-library/react';

const entry = resolve(import.meta.dirname, '../dist/index.cjs');
const built = existsSync(entry);

describe.skipIf(!built)('dist/index.cjs', () => {
  it('starts the WASM engine and evaluates a rule when required', async () => {
    const require = createRequire(import.meta.url);
    const lib = require(entry) as typeof import('../src/lib');
    expect(typeof lib.DataLogicEditor).toBe('function');

    const { result } = renderHook(() => lib.useWasmEvaluator());
    await waitFor(() => expect(result.current.loading).toBe(false), { timeout: 10_000 });
    expect(result.current.error).toBeNull();
    expect(result.current.ready).toBe(true);
    expect(result.current.evaluate({ '+': [1, { var: 'x' }] }, { x: 2 })).toBe(3);
  });
});
