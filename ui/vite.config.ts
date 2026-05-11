import { defineConfig } from 'vite'
import react from '@vitejs/plugin-react'
import wasm from 'vite-plugin-wasm'
import { resolve } from 'path'

// https://vite.dev/config/
export default defineConfig({
  base: process.env.GITHUB_ACTIONS ? '/datalogic-rs/playground/' : '/',
  plugins: [
    react(),
    wasm(),
  ],
  resolve: {
    alias: {
      '@': resolve(__dirname, 'src'),
      '@logic-editor': resolve(__dirname, 'src/components/logic-editor'),
      // Resolve the WASM dep to a vendored copy inside the UI tree
      // (`vendor/datalogic/`, kept fresh by `npm run sync-wasm`). This
      // keeps the file under the UI project root so Vite's default
      // `server.fs.allow` covers it — no need to widen the allow-list
      // out to the monorepo root.
      '@goplasmatic/datalogic': resolve(__dirname, 'vendor/datalogic'),
    },
  },
  optimizeDeps: {
    exclude: ['@goplasmatic/datalogic'],
  },
})
