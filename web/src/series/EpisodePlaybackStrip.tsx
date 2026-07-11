import type { EpisodeStrip } from './episodeStrip'

// Fixed plot height; each bar grows from the baseline as a share of it.
const TRACK_HEIGHT = 'h-28'

// A single horizontal strip of per-episode bars, ordered season → episode, with
// each season grouped under its own label. Scrolls horizontally for long runs.
export function EpisodePlaybackStrip({ strip }: { strip: EpisodeStrip }) {
  return (
    <div data-testid="episode-playback-strip" className="overflow-x-auto pb-1">
      <div className="flex items-end gap-3">
        {strip.seasons.map((season) => (
          <div key={season.seasonNumber} className="flex flex-col gap-1">
            <div className={`flex items-end gap-0.5 ${TRACK_HEIGHT}`}>
              {season.bars.map((bar) => (
                <div
                  key={bar.episodeNumber}
                  title={bar.tooltip}
                  data-testid="episode-bar"
                  className="flex h-full w-2 items-end"
                >
                  <div
                    className={`min-h-[2px] w-full rounded-t-sm ${bar.colorClass}`}
                    style={{ height: `${bar.heightPercent}%` }}
                  />
                </div>
              ))}
            </div>
            <div className="border-t border-slate-800 pt-1 text-center text-xs tabular-nums text-slate-500">
              S{season.seasonNumber}
            </div>
          </div>
        ))}
      </div>
    </div>
  )
}
