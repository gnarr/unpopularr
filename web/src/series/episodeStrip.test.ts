import { describe, expect, it } from 'vitest'
import type { PlaybackMetrics, SeriesEpisodeDetail, SeriesSeasonDetail } from '../api/types'
import { buildEpisodeStrip, episodeBarClass } from './episodeStrip'

const NOW = new Date('2026-07-01T00:00:00Z').getTime()
const DAY_MS = 24 * 60 * 60 * 1000

function episode(overrides: Partial<SeriesEpisodeDetail> = {}): SeriesEpisodeDetail {
  return {
    episodeNumber: 1,
    title: 'Ep',
    airDateUtc: '2026-01-01T00:00:00Z',
    hasFile: true,
    sizeOnDiskBytes: 0,
    playback: null,
    ...overrides,
  }
}

function season(seasonNumber: number, episodes: SeriesEpisodeDetail[]): SeriesSeasonDetail {
  const withFiles = episodes.filter((e) => e.hasFile).length
  return {
    seasonNumber,
    fileCount: withFiles,
    episodeCount: episodes.length,
    episodesWithFiles: withFiles,
    sizeOnDiskBytes: 0,
    playback: null,
    episodes,
  }
}

const watched = (
  seconds: number,
  lastPlayedAt: string | null,
  playCount = 1,
): PlaybackMetrics => ({ playCount, playDurationSeconds: seconds, lastPlayedAt })

const neverWatched: PlaybackMetrics = { playCount: 0, playDurationSeconds: 0, lastPlayedAt: null }

describe('buildEpisodeStrip', () => {
  it('preserves season → episode order and drops unaired episodes and empty seasons', () => {
    const strip = buildEpisodeStrip(
      [
        season(1, [
          episode({ episodeNumber: 1, playback: watched(60, null) }),
          // Unaired (future air date, no file) — excluded from the strip.
          episode({ episodeNumber: 2, hasFile: false, airDateUtc: '2026-12-01T00:00:00Z' }),
        ]),
        // Whole season unaired → dropped entirely.
        season(2, [episode({ episodeNumber: 1, hasFile: false, airDateUtc: null })]),
      ],
      NOW,
    )
    expect(strip.seasons.map((s) => s.seasonNumber)).toEqual([1])
    expect(strip.seasons[0].bars.map((b) => b.episodeNumber)).toEqual([1])
  })

  it('includes watched episodes without files or reliable release dates', () => {
    const strip = buildEpisodeStrip(
      [
        season(1, [
          episode({
            hasFile: false,
            airDateUtc: null,
            playback: watched(60, '2026-06-20T00:00:00Z'),
          }),
        ]),
      ],
      NOW,
    )

    expect(strip.seasons[0].bars).toHaveLength(1)
    expect(strip.hasWatchData).toBe(true)
  })

  it('normalizes heights to the most-watched episode and zeroes the unwatched', () => {
    const strip = buildEpisodeStrip(
      [
        season(1, [
          episode({ episodeNumber: 1, playback: watched(1800, '2026-06-20T00:00:00Z') }),
          episode({ episodeNumber: 2, playback: watched(900, '2026-06-20T00:00:00Z') }),
          episode({ episodeNumber: 3, playback: neverWatched }),
        ]),
      ],
      NOW,
    )
    const bars = strip.seasons[0].bars
    expect(bars[0].heightPercent).toBe(100)
    expect(bars[1].heightPercent).toBe(50)
    expect(bars[2].heightPercent).toBe(0)
    expect(bars[0].minutes).toBe(30)
    expect(strip.hasWatchData).toBe(true)
  })

  it('reports no watch data when nothing has been played', () => {
    const strip = buildEpisodeStrip([season(1, [episode({ playback: neverWatched })])], NOW)
    expect(strip.hasWatchData).toBe(false)
    expect(strip.seasons[0].bars[0].heightPercent).toBe(0)
  })
})

describe('episodeBarClass', () => {
  it('ramps watched episodes by recency and flags the rest', () => {
    expect(episodeBarClass(episode({ playback: watched(60, iso(5)) }), NOW)).toBe('bg-indigo-500/70')
    expect(episodeBarClass(episode({ playback: watched(60, iso(90)) }), NOW)).toBe('bg-indigo-500/40')
    expect(episodeBarClass(episode({ playback: watched(60, iso(300)) }), NOW)).toBe('bg-indigo-500/20')
    // Watched but without a timestamp falls back to the oldest tier.
    expect(episodeBarClass(episode({ playback: watched(60, null) }), NOW)).toBe('bg-indigo-500/20')
    // On disk but never watched → red; aired-but-missing → neutral.
    expect(episodeBarClass(episode({ playback: neverWatched }), NOW)).toBe('bg-red-500/40')
    expect(
      episodeBarClass(episode({ hasFile: false, playback: neverWatched }), NOW),
    ).toBe('bg-slate-700/50')
  })
})

function iso(daysAgo: number): string {
  return new Date(NOW - daysAgo * DAY_MS).toISOString()
}
