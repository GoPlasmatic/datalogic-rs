import { defineConfig } from 'vite'
import react from '@vitejs/plugin-react'
import wasm from 'vite-plugin-wasm'
import topLevelAwait from 'vite-plugin-top-level-await'
import { resolve } from 'path'

// https://vite.dev/config/
export default defineConfig({
  base: process.env.GITHUB_ACTIONS ? '/datalogic-rs/playground/' : '/',
  plugins: [
    react(),
    wasm(),
    topLevelAwait(),
  ],
  resolve: {
    alias: {
      '@': resolve(__dirname, 'src'),
      '@logic-editor': resolve(__dirname, 'src/components/logic-editor'),
    },
  },
  optimizeDeps: {
    exclude: ['@goplasmatic/datalogic'],
  },
})
