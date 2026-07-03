import { Link, useNavigate, useParams } from 'react-router'
import { useSeries } from '../api/queries'
import { ApiError } from '../api/http'
import type { SeriesDetails } from '../api/types'
import { Button } from '../components/Button'
import { EmptyState } from '../components/EmptyState'
import { InstanceChips } from '../components/InstanceChips'
import { StatCard } from '../components/StatCard'
import { StatCardSkeleton, TableSkeleton } from '../components/Skeletons'
import { TypeBadge } from '../components/TypeBadge'
import { absoluteTime, formatBytes, formatDuration, relativeTime } from '../lib/format'

export function SeriesDetailView() {
  const { tvdbId: raw } = useParams()
  const navigate = useNavigate()
  const tvdbId = Number(raw)
  const series = useSeries(tvdbId)

  if (!Number.isFinite(tvdbId)) return <NotFound raw={raw} onBack={() => navigate('/')} />

  if (series.isPending) {
    return (
      <div className="space-y-6">
        <div className="grid grid-cols-2 gap-4 sm:grid-cols-4">
          {Array.from({ length: 4 }).map((_, index) => (
            <StatCardSkeleton key={index} />
          ))}
        </div>
        <TableSkeleton />
      </div>
    )
  }

  if (series.isError) {
    if (series.error instanceof ApiError && series.error.status === 404) {
      return <NotFound raw={raw} onBack={() => navigate('/')} />
    }
    return (
      <EmptyState
        title="Couldn't load this series"
        description={series.error instanceof Error ? series.error.message : 'Unknown error'}
        action={<Button onClick={() => series.refetch()}>Retry</Button>}
      />
    )
  }

  return <SeriesDetail data={series.data} />
}

function NotFound({ raw, onBack }: { raw?: string; onBack: () => void }) {
  return (
    <EmptyState
      title="Series not found"
      description={`No series with TVDB id ${raw ?? ''}.`}
      action={<Button onClick={onBack}>Back to catalog</Button>}
    />
  )
}

function SeriesDetail({ data }: { data: SeriesDetails }) {
  const { playback } = data
  const neverPlayed = playback !== null && playback.playCount === 0

  return (
    <div className="space-y-6">
      <Link to="/" className="inline-block text-sm text-slate-400 hover:text-slate-200">
        ← Back to catalog
      </Link>

      <section className="rounded-lg border border-slate-800 bg-slate-900/40">
        <header className="flex flex-wrap items-center gap-3 border-b border-slate-800 px-4 py-3">
          <TypeBadge type="series" />
          <h1 className="text-lg font-semibold text-slate-100">
            {data.displayName} <span className="text-slate-500">({data.year})</span>
          </h1>
          <div className="ml-auto">
            <InstanceChips instances={data.instances} />
          </div>
        </header>
        <div className="grid grid-cols-2 gap-4 p-4 sm:grid-cols-4">
          <StatCard label="Total size" value={formatBytes(data.sizeOnDiskBytes)} />
          <StatCard label="Files" value={data.fileCount.toLocaleString()} />
          {playback !== null && (
            <>
              <StatCard
                label="Plays"
                value={neverPlayed ? 'Never' : playback.playCount.toLocaleString()}
                accent={neverPlayed}
              />
              <StatCard
                label="Watch time"
                value={neverPlayed ? '—' : formatDuration(playback.playDurationSeconds)}
              />
            </>
          )}
        </div>
        {playback?.lastPlayedAt && (
          <div className="border-t border-slate-800 px-4 py-2 text-sm text-slate-400">
            Last played{' '}
            <span className="text-slate-200" title={absoluteTime(playback.lastPlayedAt)}>
              {relativeTime(playback.lastPlayedAt)}
            </span>
          </div>
        )}
      </section>

      <section className="rounded-lg border border-slate-800 bg-slate-900/40">
        <header className="flex items-center justify-between border-b border-slate-800 px-4 py-3">
          <h2 className="text-sm font-semibold text-slate-100">Seasons</h2>
        </header>
        <div className="p-4">
          {data.seasons.length === 0 ? (
            <p className="text-sm text-slate-400">No season file data.</p>
          ) : (
            <table className="w-full border-separate border-spacing-0 text-sm">
              <thead>
                <tr>
                  <th className="border-b border-slate-800 px-3 py-2 text-left text-xs font-semibold uppercase tracking-wide text-slate-400">
                    Season
                  </th>
                  <th className="border-b border-slate-800 px-3 py-2 text-right text-xs font-semibold uppercase tracking-wide text-slate-400">
                    Files
                  </th>
                </tr>
              </thead>
              <tbody>
                {data.seasons.map((season) => (
                  <tr key={season.seasonNumber} className="hover:bg-slate-800/30">
                    <td className="border-b border-slate-800/60 px-3 py-1.5">
                      Season {season.seasonNumber}
                    </td>
                    <td className="border-b border-slate-800/60 px-3 py-1.5 text-right tabular-nums">
                      {season.fileCount.toLocaleString()}
                    </td>
                  </tr>
                ))}
              </tbody>
            </table>
          )}
        </div>
      </section>
    </div>
  )
}
