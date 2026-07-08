import { describe, expect, it } from 'vitest'
import type { SeriesEpisodeDetail } from '../api/types'
import {
  episodeCellState,
  episodeTooltip,
  isAired,
  recencyTier,
  releasedEpisodeCount,
} from './episodeDisplay'

const NOW = new Date('2026-07-01T00:00:00Z').getTime()
const DAY_MS = 24 * 60 * 60 * 1000

function episode(overrides: Partial<SeriesEpisodeDetail> = {}): SeriesEpisodeDetail {
  return {
    episodeNumber: 1,
    title: 'Pilot',
    airDateUtc: '2026-01-01T00:00:00Z',
    hasFile: true,
    sizeOnDiskBytes: 512 * 1024 * 1024,
    playback: null,
    ...overrides,
  }
}

describe('episodeCellState', () => {
  it('distinguishes playback-unavailable from never-watched', () => {
    expect(episodeCellState(episode({ playback: null }), NOW)).toBe('onDiskNoPlaybackData')
    expect(
      episodeCellState(
        episode({ playback: { playCount: 0, playDurationSeconds: 0, lastPlayedAt: null } }),
        NOW,
      ),
    ).toBe('onDiskNeverWatched')
    expect(
      episodeCellState(
        episode({
          playback: { playCount: 2, playDurationSeconds: 3600, lastPlayedAt: '2026-06-01T00:00:00Z' },
        }),
        NOW,
      ),
    ).toBe('onDiskWatched')
  })

  it('splits fileless episodes into missing and unaired at the air date', () => {
    expect(episodeCellState(episode({ hasFile: false }), NOW)).toBe('missing')
    expect(
      episodeCellState(episode({ hasFile: false, airDateUtc: '2026-07-02T00:00:00Z' }), NOW),
    ).toBe('unaired')
    expect(episodeCellState(episode({ hasFile: false, airDateUtc: null }), NOW)).toBe('unaired')
  })
})

describe('recencyTier', () => {
  it('buckets by days since last watch', () => {
    expect(recencyTier(new Date(NOW - 5 * DAY_MS).toISOString(), NOW)).toBe('recent')
    expect(recencyTier(new Date(NOW - 30 * DAY_MS).toISOString(), NOW)).toBe('recent')
    expect(recencyTier(new Date(NOW - 31 * DAY_MS).toISOString(), NOW)).toBe('stale')
    expect(recencyTier(new Date(NOW - 200 * DAY_MS).toISOString(), NOW)).toBe('old')
    expect(recencyTier('not a date', NOW)).toBe('old')
  })
})

describe('isAired / releasedEpisodeCount', () => {
  it('treats missing and future air dates as unaired', () => {
    expect(isAired(episode(), NOW)).toBe(true)
    expect(isAired(episode({ airDateUtc: null }), NOW)).toBe(false)
    expect(isAired(episode({ airDateUtc: '2026-07-02T00:00:00Z' }), NOW)).toBe(false)
  })

  it('counts on-disk early releases so the denominator never lags the files', () => {
    const episodes = [
      episode(),
      episode({ episodeNumber: 2, hasFile: false, airDateUtc: '2026-08-01T00:00:00Z' }),
      // Early release: on disk before airing still counts as released.
      episode({ episodeNumber: 3, airDateUtc: '2026-08-01T00:00:00Z' }),
    ]
    expect(releasedEpisodeCount(episodes, NOW)).toBe(2)
  })
})

describe('episodeTooltip', () => {
  it('describes an on-disk watched episode', () => {
    const detail = episode({
      playback: { playCount: 3, playDurationSeconds: 5400, lastPlayedAt: '2026-06-01T00:00:00Z' },
    })
    expect(episodeTooltip(3, detail, NOW)).toBe('S03E01 · Pilot · 512 MiB · 3 plays · watched last month')
  })

  it('describes never-watched, missing, and unaired episodes', () => {
    expect(
      episodeTooltip(
        1,
        episode({ playback: { playCount: 0, playDurationSeconds: 0, lastPlayedAt: null } }),
        NOW,
      ),
    ).toBe('S01E01 · Pilot · 512 MiB · never watched')
    expect(episodeTooltip(1, episode({ hasFile: false, sizeOnDiskBytes: 0 }), NOW)).toBe(
      'S01E01 · Pilot · missing',
    )
    expect(
      episodeTooltip(
        1,
        episode({ hasFile: false, sizeOnDiskBytes: 0, airDateUtc: null, title: '' }),
        NOW,
      ),
    ).toBe('S01E01 · unaired')
  })
})
