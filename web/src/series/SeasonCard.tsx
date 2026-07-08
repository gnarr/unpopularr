import type { SeriesSeasonDetail } from '../api/types'
import { absoluteTime, formatBytes, relativeTime } from '../lib/format'
import { EpisodeGrid } from './EpisodeGrid'
import { releasedEpisodeCount } from './episodeDisplay'

export function SeasonCard({ season }: { season: SeriesSeasonDetail }) {
  const { playback } = season
  const neverWatched =
    playback !== null && playback.playCount === 0 && season.episodesWithFiles > 0

  return (
    <section
      data-testid={`season-${season.seasonNumber}`}
      className="rounded-lg border border-slate-800 bg-slate-900/40"
    >
      <header className="flex flex-wrap items-center gap-x-3 gap-y-1 border-b border-slate-800 px-4 py-2.5">
        <h3 className="text-sm font-semibold text-slate-100">Season {season.seasonNumber}</h3>
        <span className="text-xs text-slate-400">
          {season.episodes.length > 0 ? (
            <>
              {season.episodesWithFiles}/{releasedEpisodeCount(season.episodes)} episodes ·{' '}
              {formatBytes(season.sizeOnDiskBytes)}
            </>
          ) : (
            <>
              {season.fileCount} {season.fileCount === 1 ? 'file' : 'files'}
            </>
          )}
        </span>
        <span className="ml-auto text-xs text-slate-400">
          {neverWatched && (
            <span className="rounded bg-red-500/15 px-1.5 py-0.5 font-medium text-red-300">
              Never watched
            </span>
          )}
          {playback !== null && playback.playCount > 0 && (
            <>
              {playback.playCount.toLocaleString()}{' '}
              {playback.playCount === 1 ? 'play' : 'plays'}
              {playback.lastPlayedAt && (
                <>
                  {' · watched '}
                  <span className="text-slate-200" title={absoluteTime(playback.lastPlayedAt)}>
                    {relativeTime(playback.lastPlayedAt)}
                  </span>
                </>
              )}
            </>
          )}
        </span>
      </header>
      <div className="p-3">
        {season.episodes.length > 0 ? (
          <EpisodeGrid seasonNumber={season.seasonNumber} episodes={season.episodes} />
        ) : (
          <p className="text-xs text-slate-500">Run a sync to load episode detail.</p>
        )}
      </div>
    </section>
  )
}
