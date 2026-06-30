import { defineConfig } from 'vite';
import react from '@vitejs/plugin-react';
import wasm from 'vite-plugin-wasm';
import { resolve } from 'path';

/**
 * Embed build configuration
 * Creates a standalone IIFE bundle that can be loaded via <script> tag in mdBook
 * React is bundled (not externalized) because dependencies like @xyflow/react
 * use automatic JSX transforms that require react/jsx-runtime
 */
export default defineConfig({
  plugins: [
    react(),
    wasm(),
  ],
  resolve: {
    alias: {
      '@': resolve(__dirname, 'src'),
      '@logic-editor': resolve(__dirname, 'src/components/logic-editor'),
      // Match `vite.config.ts`: resolve the WASM dep to the vendored
      // copy that `prebuild:embed` (→ sync-wasm) refreshes from
      // `../wasm/pkg/` before this build runs.
      '@goplasmatic/datalogic-wasm': resolve(__dirname, 'vendor/datalogic'),
    },
  },
  define: {
    // Ensure process.env is defined for production
    'process.env.NODE_ENV': JSON.stringify('production'),
  },
  build: {
    outDir: 'dist-embed',
    emptyOutDir: true,
    lib: {
      entry: resolve(__dirname, 'src/embed.tsx'),
      name: 'DataLogicEmbed',
      // ES module, NOT iife. wasm-bindgen's web-target init uses
      // `new URL('datalogic_wasm_bg.wasm', import.meta.url)`. Vite 8's Rolldown
      // bundler cannot represent `import.meta` in an iife and rewrites it to an
      // empty object, so `import.meta.url` becomes the string "undefined" and
      // `new URL(<inlined wasm data: URI>, "undefined")` throws
      // `TypeError: Invalid URL`, breaking WASM init. In an ES module
      // `import.meta.url` is the real, valid module URL. The bundle is loaded
      // via `<script type="module">` by docs/theme/datalogic-playground.js and
      // still exposes `window.DataLogicEmbed` as a load-time side effect.
      formats: ['es'],
      fileName: () => 'datalogic-embed.js',
    },
    rollupOptions: {
      output: {
        // Ensure CSS is bundled into a single file
        assetFileNames: (assetInfo) => {
          if (assetInfo.name?.endsWith('.css')) {
            return 'datalogic-embed.css';
          }
          return assetInfo.name ?? '[name][extname]';
        },
        // Inline dynamic imports for WASM
        inlineDynamicImports: true,
      },
    },
    // Vite 8's default minifier (oxc) is fine for the ES-module output above:
    // `import.meta` is valid ESM syntax, so it's preserved (unlike the iife
    // output, where it was rewritten to an empty object and broke WASM init).
    minify: true,
    // No sourcemap: the embed is a generated artifact and the deploy workflow
    // copies only the .js/.css (not the .map), so a `//# sourceMappingURL`
    // reference just 404s for `datalogic-embed.js.map` in the browser.
    sourcemap: false,
  },
  optimizeDeps: {
    exclude: ['@goplasmatic/datalogic-wasm'],
  },
});
