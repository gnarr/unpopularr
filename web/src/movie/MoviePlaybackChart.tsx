import { useState } from 'react'
import type { DailyPlayback } from '../api/types'
import { buildMoviePlaybackChart, RESOLUTIONS, type Resolution } from './moviePlaybackChart'

const TRACK_HEIGHT = 'h-32'

const RESOLUTION_LABELS: Record<Resolution, string> = {
  day: 'Day',
  week: 'Week',
  month: 'Month',
  year: 'Year',
}

// A continuous run of playback bars (single indigo hue — height carries the
// minutes played) with a resolution toggle. Re-bucketing happens client-side,
// so switching resolution is instant. Bars stretch to fill on short ranges and
// scroll horizontally once they hit their minimum width.
export function MoviePlaybackChart({
  dailyPlayback,
  availableAt,
}: {
  dailyPlayback: DailyPlayback[]
  availableAt: string | null
}) {
  const [resolution, setResolution] = useState<Resolution>('month')
  const chart = buildMoviePlaybackChart(dailyPlayback, availableAt, resolution)
  const first = chart.bars.at(0)
  const last = chart.bars.at(-1)

  return (
    <div data-testid="movie-playback-chart">
      <div className="mb-3 flex justify-end">
        <div
          role="group"
          aria-label="Chart resolution"
          className="inline-flex rounded-md border border-slate-800 p-0.5 text-xs"
        >
          {RESOLUTIONS.map((option) => (
            <button
              key={option}
              type="button"
              onClick={() => setResolution(option)}
              aria-pressed={resolution === option}
              className={`rounded px-2.5 py-1 font-medium ${
                resolution === option
                  ? 'bg-indigo-500/20 text-indigo-200'
                  : 'text-slate-400 hover:text-slate-200'
              }`}
            >
              {RESOLUTION_LABELS[option]}
            </button>
          ))}
        </div>
      </div>

      <div className="overflow-x-auto pb-1">
        <div className={`flex min-w-full items-end gap-0.5 ${TRACK_HEIGHT}`}>
          {chart.bars.map((bar) => (
            <div key={bar.key} title={bar.tooltip} data-testid="chart-bar" className="flex h-full min-w-[6px] flex-1 items-end">
              <div
                className="min-h-[2px] w-full rounded-t-sm bg-indigo-500/70"
                style={{ height: `${bar.heightPercent}%` }}
              />
            </div>
          ))}
        </div>
      </div>
      {first && last && (
        <div className="mt-2 flex justify-between text-[11px] tabular-nums text-slate-500">
          <span>{first.label}</span>
          <span>{last.label}</span>
        </div>
      )}
    </div>
  )
}
