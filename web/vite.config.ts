import { defineConfig } from 'vitest/config'
import react from '@vitejs/plugin-react'
import tailwindcss from '@tailwindcss/vite'

// The dev server proxies the API to the running Rust backend so the browser
// only ever talks to one origin — no CORS needed in dev or prod.
export default defineConfig({
  plugins: [react(), tailwindcss()],
  server: {
    port: 5173,
    proxy: {
      '/api': { target: 'http://127.0.0.1:3000' },
    },
  },
  build: {
    // Do not wipe the output dir: it would delete the tracked `dist/.gitkeep`
    // that guarantees the folder exists for rust-embed on a fresh checkout.
    emptyOutDir: false,
  },
  test: {
    environment: 'node',
  },
})
