import { expect, request } from '@playwright/test'

const BASE = 'http://127.0.0.1:39101'

// The app starts with an empty database (e2e/config.toml disables startup
// syncs), so populate it through the real sync endpoints. Safe to call from
// every spec file: a concurrent run answers 409 and both poll to completion.
export async function seedViaSyncs() {
  await runSync('/api/v1/sync')
  await runSync('/api/v1/playback/sync')
}

async function runSync(path: string) {
  const api = await request.newContext({ baseURL: BASE })
  const started = await api.post(path)
  expect([202, 409]).toContain(started.status())
  await expect
    .poll(async () => (await (await api.get(path)).json()).status, { timeout: 60_000 })
    .toBe('succeeded')
  await api.dispose()
}
