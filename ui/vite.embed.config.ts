import { defineConfig } from 'vite';
import react from '@vitejs/plugin-react';
import wasm from 'vite-plugin-wasm';
import topLevelAwait from 'vite-plugin-top-level-await';
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
    topLevelAwait(),
  ],
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
      formats: ['iife'],
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
    // Minify for production using esbuild
    minify: 'esbuild',
    // Generate sourcemaps for debugging
    sourcemap: true,
  },
  optimizeDeps: {
    exclude: ['@goplasmatic/datalogic'],
  },
});
