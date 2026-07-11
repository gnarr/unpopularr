import type { SeriesEpisodeDetail, SeriesSeasonDetail } from '../api/types'
import { episodeTooltip, isAired, recencyTier } from './episodeDisplay'

// One bar in the watch-time strip: height carries minutes watched, color carries
// recency. Never-watched episodes have 0 minutes and render as a baseline tick.
export interface EpisodeBar {
  episodeNumber: number
  minutes: number
  // 0–100, this bar's share of the tallest bar in the strip.
  heightPercent: number
  colorClass: string
  tooltip: string
}

export interface SeasonBars {
  seasonNumber: number
  bars: EpisodeBar[]
}

export interface EpisodeStrip {
  seasons: SeasonBars[]
  // False when playback is available but nothing has been watched — the caller
  // hides the section rather than render a flat baseline.
  hasWatchData: boolean
}

// Watched → the season matrix's single-hue indigo ramp (brighter = more recent);
// never-watched-on-disk wears the app's deletion red; an aired episode with no
// file is neutral. Height, not color, encodes minutes watched.
export function episodeBarClass(
  episode: SeriesEpisodeDetail,
  now: number = Date.now(),
): string {
  const { playback } = episode
  if (playback !== null && playback.playCount > 0) {
    const tier = playback.lastPlayedAt ? recencyTier(playback.lastPlayedAt, now) : 'old'
    if (tier === 'recent') return 'bg-indigo-500/70'
    if (tier === 'stale') return 'bg-indigo-500/40'
    return 'bg-indigo-500/20'
  }
  return episode.hasFile ? 'bg-red-500/40' : 'bg-slate-700/50'
}

// Flatten seasons → episodes (both already ascending) into one ordered strip.
// Released or watched episodes get a bar; otherwise unaired episodes would only
// pad the axis. Heights are normalized to the most-watched episode across the
// whole series.
export function buildEpisodeStrip(
  seasons: SeriesSeasonDetail[],
  now: number = Date.now(),
): EpisodeStrip {
  const released = seasons.map((season) => ({
    seasonNumber: season.seasonNumber,
    episodes: season.episodes.filter(
      (episode) =>
        episode.hasFile || isAired(episode, now) || (episode.playback?.playCount ?? 0) > 0,
    ),
  }))
  const maxSeconds = Math.max(
    0,
    ...released.flatMap((season) =>
      season.episodes.map((episode) => episode.playback?.playDurationSeconds ?? 0),
    ),
  )
  const hasWatchData = released.some((season) =>
    season.episodes.some((episode) => (episode.playback?.playCount ?? 0) > 0),
  )
  const seasonBars = released
    .filter((season) => season.episodes.length > 0)
    .map((season) => ({
      seasonNumber: season.seasonNumber,
      bars: season.episodes.map((episode) => {
        const seconds = episode.playback?.playDurationSeconds ?? 0
        return {
          episodeNumber: episode.episodeNumber,
          minutes: seconds / 60,
          heightPercent: maxSeconds > 0 ? (seconds / maxSeconds) * 100 : 0,
          colorClass: episodeBarClass(episode, now),
          tooltip: episodeTooltip(season.seasonNumber, episode, now),
        }
      }),
    }))

  return { seasons: seasonBars, hasWatchData }
}
