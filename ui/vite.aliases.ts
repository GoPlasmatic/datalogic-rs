import { resolve } from 'node:path';

const root = import.meta.dirname;

/**
 * Module aliases shared by every Vite and Vitest config.
 *
 * `@goplasmatic/datalogic-wasm` always resolves to the vendored copy in
 * `vendor/datalogic/`, which `npm run sync-wasm` refreshes from
 * `../bindings/wasm/pkg/`. node_modules never feeds a build: the UI runs
 * against the engine built from this repository. Keeping the copy under the
 * UI root also means Vite's default `server.fs.allow` covers it.
 *
 * - `browser` (the app, library and embed builds) resolves the package
 *   through its `package.json`, which picks the `web` target.
 * - `node` (Vitest) uses the `nodejs` target, which loads the `.wasm`
 *   synchronously from disk. The bare specifier goes through
 *   `tests/wasm-node-shim.ts`, which adds the web target's async default
 *   loader so `useWasmEvaluator` runs unchanged. `/nodejs` must come first:
 *   an alias also matches longer specifiers that start with its key.
 *
 * The `paths` blocks in `tsconfig.app.json` and `tsconfig.lib.json` mirror
 * these for the type checker; keep them in step.
 */
export function aliases(wasmTarget: 'browser' | 'node'): Record<string, string> {
  const shared = {
    '@': resolve(root, 'src'),
    '@logic-editor': resolve(root, 'src/components/logic-editor'),
  };
  if (wasmTarget === 'node') {
    return {
      ...shared,
      '@goplasmatic/datalogic-wasm/nodejs': resolve(root, 'vendor/datalogic/nodejs/datalogic_wasm.js'),
      '@goplasmatic/datalogic-wasm': resolve(root, 'tests/wasm-node-shim.ts'),
    };
  }
  return {
    ...shared,
    '@goplasmatic/datalogic-wasm': resolve(root, 'vendor/datalogic'),
  };
}
