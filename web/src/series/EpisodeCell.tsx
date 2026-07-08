import type { SeriesEpisodeDetail } from '../api/types'
import {
  episodeCellClass,
  episodeCellState,
  episodeTooltip,
  isWatched,
  recencyTier,
} from './episodeDisplay'

export function EpisodeCell({
  seasonNumber,
  episode,
}: {
  seasonNumber: number
  episode: SeriesEpisodeDetail
}) {
  const state = episodeCellState(episode)
  const tier =
    state === 'onDiskWatched' && episode.playback?.lastPlayedAt
      ? recencyTier(episode.playback.lastPlayedAt)
      : undefined

  return (
    <div
      title={episodeTooltip(seasonNumber, episode)}
      className={`relative flex h-8 items-center justify-center rounded text-xs font-medium tabular-nums select-none ${episodeCellClass(state, tier)}`}
    >
      {episode.episodeNumber}
      {state === 'missing' && isWatched(episode) && (
        <span
          className="absolute top-0.5 right-0.5 h-1.5 w-1.5 rounded-full bg-indigo-400"
          aria-hidden
        />
      )}
    </div>
  )
}
