import { defineConfig } from 'vitest/config';
import { resolve } from 'path';

// Unit tests run under Node. The WASM engine is reachable through the
// vendored nodejs target (see `npm run sync-wasm`), aliased below so tests
// can `import * as wasm from '@goplasmatic/datalogic-wasm/nodejs'` without
// going through the browser loader the app uses.
export default defineConfig({
  resolve: {
    alias: {
      '@': resolve(__dirname, 'src'),
      '@logic-editor': resolve(__dirname, 'src/components/logic-editor'),
      '@goplasmatic/datalogic-wasm/nodejs': resolve(__dirname, 'vendor/datalogic/nodejs/datalogic_wasm.js'),
      '@goplasmatic/datalogic-wasm': resolve(__dirname, 'vendor/datalogic/nodejs/datalogic_wasm.js'),
    },
  },
  test: {
    environment: 'node',
    include: ['src/**/*.test.{ts,tsx}', 'tests/**/*.test.{ts,tsx}'],
    globals: false,
  },
});
