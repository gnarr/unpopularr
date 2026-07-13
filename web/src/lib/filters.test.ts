import { describe, expect, it } from 'vitest'
import type { ContentType, MovieItem } from '../api/types'
import { matchesFilters, type CatalogFilters } from './filters'

function movie(overrides: Partial<MovieItem> = {}): MovieItem {
  return {
    contentType: 'movie',
    displayName: 'The Matrix',
    sizeOnDiskBytes: 100,
    fileCount: 1,
    instances: [
      {
        id: 'radarr-hd',
        name: 'Radarr HD',
        lastSuccessfulSyncAt: '2026-01-01T00:00:00Z',
        deepLinkPath: 'movie/the-matrix-603',
      },
    ],
    playback: null,
    tmdbId: 603,
    year: 1999,
    ...overrides,
  }
}

const ALL_TYPES = new Set<ContentType>(['movie', 'series', 'artist'])

function filters(overrides: Partial<CatalogFilters> = {}): CatalogFilters {
  return {
    search: '',
    types: ALL_TYPES,
    instanceIds: new Set(),
    neverPlayedOnly: false,
    ...overrides,
  }
}

describe('matchesFilters', () => {
  it('passes with default filters', () => {
    expect(matchesFilters(movie(), filters())).toBe(true)
  })

  it('filters by content type', () => {
    expect(matchesFilters(movie(), filters({ types: new Set(['series']) }))).toBe(false)
  })

  it('matches search case-insensitively', () => {
    expect(matchesFilters(movie(), filters({ search: 'matrix' }))).toBe(true)
    expect(matchesFilters(movie(), filters({ search: 'inception' }))).toBe(false)
  })

  it('treats an empty instance set as "all"', () => {
    expect(matchesFilters(movie(), filters({ instanceIds: new Set() }))).toBe(true)
  })

  it('filters by instance membership', () => {
    expect(matchesFilters(movie(), filters({ instanceIds: new Set(['radarr-hd']) }))).toBe(true)
    expect(matchesFilters(movie(), filters({ instanceIds: new Set(['radarr-4k']) }))).toBe(false)
  })

  it('applies the never-played filter only to available playback', () => {
    const neverPlayed = movie({
      playback: { playCount: 0, playDurationSeconds: 0, lastPlayedAt: null },
    })
    const played = movie({
      playback: { playCount: 2, playDurationSeconds: 50, lastPlayedAt: '2026-01-01T00:00:00Z' },
    })
    expect(matchesFilters(neverPlayed, filters({ neverPlayedOnly: true }))).toBe(true)
    expect(matchesFilters(played, filters({ neverPlayedOnly: true }))).toBe(false)
    expect(matchesFilters(movie(), filters({ neverPlayedOnly: true }))).toBe(false)
  })
})
