import { defineConfig } from 'vite';
import react from '@vitejs/plugin-react';
import dts from 'vite-plugin-dts';
import wasm from 'vite-plugin-wasm';
import topLevelAwait from 'vite-plugin-top-level-await';
import { resolve } from 'path';

// Library build configuration
export default defineConfig({
  plugins: [
    react(),
    wasm(),
    topLevelAwait(),
    dts({
      tsconfigPath: './tsconfig.lib.json',
      rollupTypes: true,
      outDir: 'dist',
    }),
  ],
  resolve: {
    alias: {
      '@': resolve(__dirname, 'src'),
      '@logic-editor': resolve(__dirname, 'src/components/logic-editor'),
    },
  },
  build: {
    lib: {
      entry: resolve(__dirname, 'src/lib.ts'),
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
        // Global names for UMD build
        globals: {
          react: 'React',
          'react-dom': 'ReactDOM',
          'react/jsx-runtime': 'ReactJSXRuntime',
          '@xyflow/react': 'ReactFlow',
        },
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
