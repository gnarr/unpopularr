import { expect, test } from '@playwright/test'
import { seedViaSyncs } from './seed'

const ARTIST_MBID = 'aaaaaaaa-1111-2222-3333-444444444444'

test.beforeAll(seedViaSyncs)

test('catalog links through to the artist details page', async ({ page }) => {
  await page.goto('/')
  await page.getByRole('link', { name: 'Mock Artist' }).click()
  await expect(page).toHaveURL(new RegExp(`/artists/${ARTIST_MBID}$`))
  await expect(page.getByRole('heading', { name: /Mock Artist/ })).toBeVisible()
})

test('shows artist stats and the albums table', async ({ page }) => {
  await page.goto(`/artists/${ARTIST_MBID}`)

  // Artists have no year — the heading is just the name.
  await expect(page.getByRole('heading', { name: 'Mock Artist', exact: true })).toBeVisible()

  // Stat cards: 4800 B across 12 files, and the single 240 s track play.
  await expect(page.getByText('4.7 KiB')).toBeVisible()
  await expect(page.getByText('12', { exact: true })).toBeVisible()
  await expect(page.getByText('4m 0s')).toBeVisible()
  await expect(page.getByText('Last played')).toBeVisible()

  // Albums with files, each with summed size and file count.
  await expect(page.getByRole('heading', { name: 'Albums' })).toBeVisible()
  const first = page.getByRole('row', { name: /First Album/ })
  await expect(first).toContainText('3.9 KiB')
  await expect(first).toContainText('10')
  const second = page.getByRole('row', { name: /Second Album/ })
  await expect(second).toContainText('800 B')
  await expect(second).toContainText('2')

  // The zero-file album is skipped at sync time and never shown.
  await expect(page.getByText('Empty Album')).toHaveCount(0)

  // A single-instance setup skips the per-instance breakdown.
  await expect(page.getByRole('heading', { name: 'Instances' })).toHaveCount(0)

  await page.screenshot({ path: 'test-results/artist-details.png', fullPage: true })
})
