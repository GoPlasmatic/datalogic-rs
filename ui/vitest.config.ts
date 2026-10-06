import { defineConfig } from 'vitest/config';
import { aliases } from './vite.aliases';

// Unit tests run under Node. The WASM engine is reachable through the
// vendored nodejs target (see `npm run sync-wasm`), aliased so tests can
// `import * as wasm from '@goplasmatic/datalogic-wasm'` without going through
// the browser loader the app uses.
export default defineConfig({
  resolve: {
    alias: aliases('node'),
  },
  test: {
    environment: 'node',
    include: ['src/**/*.test.{ts,tsx}', 'tests/**/*.test.{ts,tsx}'],
    globals: false,
  },
});
