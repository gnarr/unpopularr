import { describe, expect, it } from 'vitest'
import type { ArtistItem, MovieItem, SeriesItem } from '../api/types'
import {
  detailCount,
  detailLabel,
  isNeverPlayed,
  playbackAvailable,
  stableId,
  year,
} from './content'

const movie: MovieItem = {
  contentType: 'movie',
  displayName: 'Heat',
  sizeOnDiskBytes: 100,
  fileCount: 1,
  instances: [],
  playback: null,
  tmdbId: 949,
  year: 1995,
}

const series: SeriesItem = {
  contentType: 'series',
  displayName: 'The Wire',
  sizeOnDiskBytes: 200,
  fileCount: 60,
  instances: [],
  playback: null,
  tvdbId: 79126,
  year: 2002,
  seasonsWithFiles: 5,
}

const artist: ArtistItem = {
  contentType: 'artist',
  displayName: 'Boards of Canada',
  sizeOnDiskBytes: 300,
  fileCount: 40,
  instances: [],
  playback: null,
  musicBrainzId: 'mbid-1',
  albumsWithFiles: 1,
}

describe('isNeverPlayed', () => {
  it('is false when playback is unavailable (null)', () => {
    expect(isNeverPlayed({ ...movie, playback: null })).toBe(false)
  })

  it('is true only when available with zero plays', () => {
    expect(
      isNeverPlayed({
        ...movie,
        playback: { playCount: 0, playDurationSeconds: 0, lastPlayedAt: null },
      }),
    ).toBe(true)
  })

  it('is false when played at least once', () => {
    expect(
      isNeverPlayed({
        ...movie,
        playback: { playCount: 3, playDurationSeconds: 100, lastPlayedAt: null },
      }),
    ).toBe(false)
  })
})

describe('playbackAvailable', () => {
  it('is false when every item has null playback', () => {
    expect(playbackAvailable([movie, series])).toBe(false)
  })

  it('is true when any item has playback', () => {
    expect(
      playbackAvailable([
        movie,
        { ...series, playback: { playCount: 0, playDurationSeconds: 0, lastPlayedAt: null } },
      ]),
    ).toBe(true)
  })
})

describe('detail helpers and ids', () => {
  it('reports type-specific counts', () => {
    expect(detailCount(movie)).toBeNull()
    expect(detailCount(series)).toBe(5)
    expect(detailCount(artist)).toBe(1)
    expect(detailLabel(series)).toBe('5 seasons')
    expect(detailLabel(artist)).toBe('1 album')
    expect(detailLabel(movie)).toBe('—')
  })

  it('exposes stable ids and years', () => {
    expect(stableId(movie)).toBe('949')
    expect(stableId(series)).toBe('79126')
    expect(stableId(artist)).toBe('mbid-1')
    expect(year(movie)).toBe(1995)
    expect(year(artist)).toBeNull()
  })
})
