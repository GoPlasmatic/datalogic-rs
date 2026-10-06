import { defineConfig, type Plugin } from 'vite';
import react from '@vitejs/plugin-react';
import dts from 'vite-plugin-dts';
import wasm from 'vite-plugin-wasm';
import { resolve } from 'node:path';
import { aliases } from './vite.aliases.ts';

/**
 * Give the CJS build a valid `import.meta.url`.
 *
 * wasm-bindgen's web-target loader resolves the engine with
 * `new URL(<wasm>, import.meta.url)`; in a library build Vite inlines the
 * `.wasm` as a `data:` URI. ESM output keeps `import.meta.url`, but CJS has
 * no `import.meta`, so Rolldown rewrites it to `{}` and the base becomes
 * `"undefined"`. `new URL()` validates its base even for an absolute input,
 * so it threw `TypeError: Invalid URL` and every `require` consumer
 * (Jest, CJS SSR, older bundlers) could not start the engine.
 * (`vite.embed.config.ts` avoids the same rewrite by emitting ESM only.)
 *
 * The rewrite runs after Vite has inlined the asset, and only inside the
 * vendored glue. In ESM `import.meta.url` is always a non-empty string, so
 * the fallback never runs there; in CJS it becomes `{}.url || <fallback>`.
 * The base only has to be a valid URL: the input is an absolute `data:` URI.
 */
function cjsImportMetaUrl(): Plugin {
  const vendored = /[\\/]vendor[\\/]datalogic[\\/]/;
  const fallback =
    "(typeof document !== 'undefined' && document.baseURI || 'file:///')";
  return {
    name: 'datalogic:cjs-import-meta-url',
    enforce: 'post',
    transform(code, id) {
      if (!vendored.test(id) || !code.includes('import.meta.url')) return null;
      return {
        code: code.replaceAll('import.meta.url', `(import.meta.url || ${fallback})`),
        map: null,
      };
    },
  };
}

// Library build configuration
export default defineConfig({
  plugins: [
    react(),
    wasm(),
    cjsImportMetaUrl(),
    dts({
      tsconfigPath: './tsconfig.lib.json',
      outDirs: 'dist',
    }),
  ],
  resolve: {
    alias: aliases('browser'),
  },
  build: {
    lib: {
      entry: resolve(import.meta.dirname, 'src/lib.ts'),
      name: 'DataLogicUI',
      formats: ['es', 'cjs'],
      fileName: (format) => `index.${format === 'es' ? 'js' : 'cjs'}`,
    },
    rollupOptions: {
      onwarn(warning, warn) {
        // Rolldown flags every `import.meta` in CJS output. The one in the
        // vendored glue is given a fallback by cjsImportMetaUrl() above.
        if (warning.code === 'EMPTY_IMPORT_META' && /vendor[\\/]datalogic/.test(warning.message)) {
          return;
        }
        warn(warning);
      },
      // Externalize peer dependencies
      external: [
        'react',
        'react-dom',
        'react/jsx-runtime',
        '@xyflow/react',
      ],
      output: {
        // Ensure CSS is bundled into a single file with consistent name
        assetFileNames: (assetInfo) => {
          if (assetInfo.name?.endsWith('.css')) {
            return 'styles.css';
          }
          return assetInfo.name ?? 'assets/[name][extname]';
        },
      },
    },
    sourcemap: true,
    // Don't minify for better debugging
    minify: false,
  },
});
