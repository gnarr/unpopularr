import { expect, test } from '@playwright/test'
import { seedViaSyncs } from './seed'

test.beforeAll(seedViaSyncs)

test('catalog links through to the series details page', async ({ page }) => {
  await page.goto('/')
  await page.getByRole('link', { name: 'Mock Show' }).click()
  await expect(page).toHaveURL(/\/series\/7777$/)
  await expect(page.getByRole('heading', { name: /Mock Show/ })).toBeVisible()
})

test('season cards render the episode matrix with watch states', async ({ page }) => {
  await page.goto('/series/7777')
  await expect(page.getByRole('heading', { name: /Mock Show/ })).toBeVisible()

  // Legend appears once above the season cards.
  await expect(page.getByText('Watched (bright = recent)')).toBeVisible()

  // Season 1: 2 of 3 released episodes on disk, sizes summed from episode files.
  const season1 = page.getByTestId('season-1')
  await expect(season1.getByRole('heading', { name: 'Season 1' })).toBeVisible()
  await expect(season1.getByText('2/3 episodes · 2.1 KiB')).toBeVisible()
  await expect(season1.getByText('2 plays')).toBeVisible()

  // Watched episode: indigo cell with play stats in the tooltip.
  const pilot = season1.getByTitle(/^S01E01/)
  await expect(pilot).toHaveAttribute('title', /Pilot · 1000 B · 1 play · watched/)
  await expect(pilot).toHaveClass(/bg-indigo-500/)

  // On disk but never watched: the deletion-candidate red tint.
  const growth = season1.getByTitle(/^S01E02/)
  await expect(growth).toHaveAttribute('title', /Growth · 1.2 KiB · never watched/)
  await expect(growth).toHaveClass(/bg-red-500\/15/)

  // Watched then deleted: missing cell that still reports its plays.
  const deleted = season1.getByTitle(/^S01E03/)
  await expect(deleted).toHaveAttribute('title', /Deleted One · missing · 1 play/)

  // Season 2: on-disk episode never watched -> chip; unaired finale is dashed.
  const season2 = page.getByTestId('season-2')
  await expect(season2.getByText('Never watched')).toBeVisible()
  const finale = season2.getByTitle(/^S02E02/)
  await expect(finale).toHaveAttribute('title', /Unaired Finale · unaired/)
  await expect(finale).toHaveClass(/border-dashed/)

  // Specials are excluded from the matrix entirely.
  await expect(page.getByTestId('season-0')).toHaveCount(0)

  // The legacy play without episode positions surfaces in the footnote.
  await expect(page.getByText("1 play isn't shown per episode")).toBeVisible()

  await page.screenshot({ path: 'test-results/series-details.png', fullPage: true })
})
