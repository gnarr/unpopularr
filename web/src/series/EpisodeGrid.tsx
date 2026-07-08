import type { SeriesEpisodeDetail } from '../api/types'
import { EpisodeCell } from './EpisodeCell'

// Fixed-size tracks (auto-fill, capped) so a 2-episode season and a 60-episode
// season render the same cell size and wrap naturally on narrow screens.
export function EpisodeGrid({
  seasonNumber,
  episodes,
}: {
  seasonNumber: number
  episodes: SeriesEpisodeDetail[]
}) {
  return (
    <div className="grid grid-cols-[repeat(auto-fill,minmax(2.5rem,3rem))] gap-1">
      {episodes.map((episode) => (
        <EpisodeCell key={episode.episodeNumber} seasonNumber={seasonNumber} episode={episode} />
      ))}
    </div>
  )
}
