import { expect, test } from '@playwright/test'
import { seedViaSyncs } from './seed'

test.beforeAll(seedViaSyncs)

test('catalog links through to the movie details page', async ({ page }) => {
  await page.goto('/')
  await page.getByRole('link', { name: 'Mock Movie' }).click()
  await expect(page).toHaveURL(/\/movies\/550$/)
  await expect(page.getByRole('heading', { name: /Mock Movie/ })).toBeVisible()
})

test('catalog shows how long each item has been in the library', async ({ page }) => {
  await page.goto('/')
  await expect(page.getByRole('columnheader', { name: 'Age' })).toBeVisible()

  // Radarr added the movie in January 2025, so it is years rather than days old
  // and the cell is never the unknown-age dash.
  const row = page.getByRole('row').filter({ hasText: 'Mock Movie' })
  await expect(row.getByText(/^\d+(y|mo|d|h)$/)).toBeVisible()
})

test('shows size, file, and playback stats without an instance table', async ({ page }) => {
  await page.goto('/movies/550')
  await expect(page.getByRole('heading', { name: 'Mock Movie (1999)' })).toBeVisible()

  // Stat cards: 2500 B on disk, one file, and the single 5400 s Tautulli play.
  await expect(page.getByText('Total size')).toBeVisible()
  await expect(page.getByText('2.4 KiB')).toBeVisible()
  await expect(page.getByText('Files')).toBeVisible()
  // "Plays" also names a column in the watched-by table; target the stat card.
  await expect(page.getByText('Plays', { exact: true }).first()).toBeVisible()
  await expect(page.getByText('1h 30m')).toBeVisible()
  await expect(page.getByText('Last played')).toBeVisible()
  await expect(page.getByText('Added')).toBeVisible()

  // The per-user breakdown lists the single Tautulli watcher.
  await expect(page.getByRole('heading', { name: 'Watched by' })).toBeVisible()
  await expect(page.getByRole('cell', { name: 'Alice' })).toBeVisible()

  // A single-instance setup skips the per-instance breakdown.
  await expect(page.getByRole('heading', { name: 'Instances' })).toHaveCount(0)

  await page.screenshot({ path: 'test-results/movie-details.png', fullPage: true })
})

test('renders the minutes-played chart with a resolution toggle', async ({ page }) => {
  await page.goto('/movies/550')
  await expect(page.getByRole('heading', { name: 'Minutes played' })).toBeVisible()

  const chart = page.getByTestId('movie-playback-chart')
  await expect(chart).toBeVisible()

  // Default month resolution: a continuous axis from the Radarr "added" date
  // (Jan 2025) through the current month, with the single play's month filled.
  const monthBars = await chart.getByTestId('chart-bar').count()
  expect(monthBars).toBeGreaterThan(1)

  // Played buckets carry a duration + play-count tooltip (two separators);
  // empty buckets read "… · no plays".
  await expect(chart.getByTitle(/·[^·]*·[^·]*play/).first()).toBeVisible()
  await expect(chart.getByTitle(/· no plays/).first()).toBeVisible()

  // Switching to a finer resolution re-buckets client-side into more bars.
  await page.getByRole('button', { name: 'Week' }).click()
  expect(await chart.getByTestId('chart-bar').count()).toBeGreaterThan(monthBars)
})
