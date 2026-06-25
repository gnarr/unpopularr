import { defineConfig } from 'vitest/config'
import type { Plugin } from 'vite'
import react from '@vitejs/plugin-react'
import tailwindcss from '@tailwindcss/vite'

function preserveDistGitkeep(): Plugin {
  return {
    name: 'preserve-dist-gitkeep',
    apply: 'build' as const,
    generateBundle() {
      this.emitFile({ type: 'asset', fileName: '.gitkeep', source: '' })
    },
  }
}

// The dev server proxies the API to the running Rust backend so the browser
// only ever talks to one origin — no CORS needed in dev or prod.
export default defineConfig({
  plugins: [react(), tailwindcss(), preserveDistGitkeep()],
  server: {
    port: 5173,
    proxy: {
      '/api': { target: 'http://127.0.0.1:3000' },
    },
  },
  build: {
    emptyOutDir: true,
  },
  test: {
    environment: 'node',
  },
})
