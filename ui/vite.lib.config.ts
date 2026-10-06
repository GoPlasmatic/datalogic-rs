import { defineConfig } from 'vite';
import react from '@vitejs/plugin-react';
import dts from 'vite-plugin-dts';
import wasm from 'vite-plugin-wasm';
import { resolve } from 'node:path';
import { aliases } from './vite.aliases';

// Library build configuration
export default defineConfig({
  plugins: [
    react(),
    wasm(),
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
