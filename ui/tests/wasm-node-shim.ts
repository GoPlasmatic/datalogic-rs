/**
 * `@goplasmatic/datalogic-wasm` as Vitest sees it (see vite.aliases.ts).
 *
 * The app loads the package's web target, whose default export is the async
 * loader that `useWasmEvaluator` awaits. Under Node the tests use the nodejs
 * target instead, which instantiates the engine synchronously on import and
 * has no loader. This re-exports the nodejs target with a no-op default, so
 * code written against the web target runs unchanged in tests.
 */
export * from '../vendor/datalogic/nodejs/datalogic_wasm.js';

export default async function init(): Promise<void> {}
