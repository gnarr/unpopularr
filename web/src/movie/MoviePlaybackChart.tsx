import type { MoviePlaybackChart as ChartData } from './moviePlaybackChart'

const TRACK_HEIGHT = 'h-32'

// A continuous run of monthly bars (single indigo hue — height carries the
// minutes played). Bars stretch to fill on short ranges and scroll horizontally
// once they hit their minimum width. The axis is labeled at both ends.
export function MoviePlaybackChart({ chart }: { chart: ChartData }) {
  const first = chart.bars.at(0)
  const last = chart.bars.at(-1)

  return (
    <div data-testid="movie-playback-chart">
      <div className="overflow-x-auto pb-1">
        <div className={`flex min-w-full items-end gap-0.5 ${TRACK_HEIGHT}`}>
          {chart.bars.map((bar) => (
            <div
              key={bar.month}
              title={bar.tooltip}
              data-testid="month-bar"
              className="flex h-full min-w-[6px] flex-1 items-end"
            >
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
