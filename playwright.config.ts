import { defineConfig } from '@playwright/test'

// End-to-end tests against a locally started app (e2e/servers.sh boots the
// backend in the project's Rust build image plus a mock Sonarr/Tautulli).
export default defineConfig({
  testDir: 'e2e',
  projects: [{ name: 'chromium', use: { browserName: 'chromium', headless: true } }],
  use: {
    baseURL: 'http://127.0.0.1:39101',
  },
  webServer: {
    command: 'bash e2e/servers.sh',
    url: 'http://127.0.0.1:39101/api/v1/content',
    reuseExistingServer: false,
    // Cold cargo builds take minutes; warm ones seconds.
    timeout: 600_000,
    gracefulShutdown: { signal: 'SIGTERM', timeout: 15_000 },
  },
})
