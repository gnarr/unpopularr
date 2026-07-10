import { expect, test } from '@playwright/test'
import { seedViaSyncs } from './seed'

test.beforeAll(seedViaSyncs)

test('catalog links through to the movie details page', async ({ page }) => {
  await page.goto('/')
  await page.getByRole('link', { name: 'Mock Movie' }).click()
  await expect(page).toHaveURL(/\/movies\/550$/)
  await expect(page.getByRole('heading', { name: /Mock Movie/ })).toBeVisible()
})

test('shows size, file, and playback stats without an instance table', async ({ page }) => {
  await page.goto('/movies/550')
  await expect(page.getByRole('heading', { name: 'Mock Movie (1999)' })).toBeVisible()

  // Stat cards: 2500 B on disk, one file, and the single 5400 s Tautulli play.
  await expect(page.getByText('Total size')).toBeVisible()
  await expect(page.getByText('2.4 KiB')).toBeVisible()
  await expect(page.getByText('Files')).toBeVisible()
  await expect(page.getByText('Plays', { exact: true })).toBeVisible()
  await expect(page.getByText('1h 30m')).toBeVisible()
  await expect(page.getByText('Last played')).toBeVisible()

  // A single-instance setup skips the per-instance breakdown.
  await expect(page.getByRole('heading', { name: 'Instances' })).toHaveCount(0)

  await page.screenshot({ path: 'test-results/movie-details.png', fullPage: true })
})
