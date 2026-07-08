import type { SeriesEpisodeDetail } from '../api/types'
import { formatBytes, relativeTime } from '../lib/format'

// Visual state of one episode cell in the season matrix. `playback: null`
// (unavailable) and playCount 0 (never watched) are distinct — the same
// load-bearing distinction as isNeverPlayed in lib/content.ts.
export type EpisodeCellState =
  | 'unaired'
  | 'missing'
  | 'onDiskNeverWatched'
  | 'onDiskWatched'
  | 'onDiskNoPlaybackData'

// How recently a watched episode was last played, driving the accent ramp.
export type RecencyTier = 'recent' | 'stale' | 'old'

const DAY_MS = 24 * 60 * 60 * 1000
const RECENT_DAYS = 30
const STALE_DAYS = 180

export function episodeCellState(
  episode: SeriesEpisodeDetail,
  now: number = Date.now(),
): EpisodeCellState {
  if (!episode.hasFile) {
    return isAired(episode, now) ? 'missing' : 'unaired'
  }
  if (episode.playback === null) return 'onDiskNoPlaybackData'
  return episode.playback.playCount > 0 ? 'onDiskWatched' : 'onDiskNeverWatched'
}

export function recencyTier(lastPlayedAt: string, now: number = Date.now()): RecencyTier {
  const timestamp = new Date(lastPlayedAt).getTime()
  if (Number.isNaN(timestamp)) return 'old'
  const days = (now - timestamp) / DAY_MS
  if (days <= RECENT_DAYS) return 'recent'
  if (days <= STALE_DAYS) return 'stale'
  return 'old'
}

// Watched at least once — used for the "watched then deleted" dot on missing
// cells, which is this app's desired end state for an episode.
export function isWatched(episode: SeriesEpisodeDetail): boolean {
  return episode.playback !== null && episode.playback.playCount > 0
}

export function isAired(episode: SeriesEpisodeDetail, now: number = Date.now()): boolean {
  if (episode.airDateUtc === null) return false
  const timestamp = new Date(episode.airDateUtc).getTime()
  return !Number.isNaN(timestamp) && timestamp <= now
}

// Denominator for "on disk / released": aired episodes, plus any on-disk
// episode regardless of air date (early releases must not exceed the total).
export function releasedEpisodeCount(
  episodes: SeriesEpisodeDetail[],
  now: number = Date.now(),
): number {
  return episodes.filter((episode) => episode.hasFile || isAired(episode, now)).length
}

// Tailwind classes per cell state. Watched cells use a single-hue sequential
// ramp (brighter = more recent); never-watched-on-disk wears the app's
// deletion-candidate red. Every state also differs by a non-color treatment
// (dashed border, ring, fill) so the matrix survives colorblindness and print.
export function episodeCellClass(state: EpisodeCellState, tier?: RecencyTier): string {
  switch (state) {
    case 'unaired':
      return 'border border-dashed border-slate-700 text-slate-600'
    case 'missing':
      return 'border border-slate-700/60 bg-slate-800/40 text-slate-500'
    case 'onDiskNoPlaybackData':
      return 'bg-slate-700/60 text-slate-200'
    case 'onDiskNeverWatched':
      return 'bg-red-500/15 text-red-300 ring-1 ring-inset ring-red-500/30'
    case 'onDiskWatched':
      switch (tier) {
        case 'recent':
          return 'bg-indigo-500/70 text-white'
        case 'stale':
          return 'bg-indigo-500/40 text-indigo-100'
        default:
          return 'bg-indigo-500/20 text-indigo-200'
      }
  }
}

export function episodeTooltip(
  seasonNumber: number,
  episode: SeriesEpisodeDetail,
  now: number = Date.now(),
): string {
  const code = `S${pad(seasonNumber)}E${pad(episode.episodeNumber)}`
  const parts = [episode.title ? `${code} · ${episode.title}` : code]

  if (episode.hasFile) {
    parts.push(formatBytes(episode.sizeOnDiskBytes))
  } else {
    parts.push(isAired(episode, now) ? 'missing' : 'unaired')
  }

  const { playback } = episode
  if (playback !== null) {
    if (playback.playCount === 0) {
      parts.push('never watched')
    } else {
      parts.push(`${playback.playCount} ${playback.playCount === 1 ? 'play' : 'plays'}`)
      if (playback.lastPlayedAt !== null) {
        parts.push(`watched ${relativeTime(playback.lastPlayedAt, now)}`)
      }
    }
  }

  return parts.join(' · ')
}

function pad(value: number): string {
  return String(value).padStart(2, '0')
}
