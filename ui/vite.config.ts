import { defineConfig } from 'vite';
import react from '@vitejs/plugin-react';
import wasm from 'vite-plugin-wasm';
import { aliases } from './vite.aliases';

// Playground app (`npm run dev`, `npm run build`).
export default defineConfig({
  base: process.env.GITHUB_ACTIONS ? '/datalogic-rs/playground/' : '/',
  plugins: [react(), wasm()],
  resolve: {
    alias: aliases('browser'),
  },
  optimizeDeps: {
    exclude: ['@goplasmatic/datalogic-wasm'],
  },
});
