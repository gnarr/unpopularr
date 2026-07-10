import { Link, useNavigate, useParams } from 'react-router'
import { useSeries } from '../api/queries'
import { ApiError } from '../api/http'
import type { SeriesDetails } from '../api/types'
import { Button } from '../components/Button'
import { DetailHeaderCard } from '../components/DetailHeaderCard'
import { EmptyState } from '../components/EmptyState'
import { InstanceTable } from '../components/InstanceTable'
import { DetailSkeleton } from '../components/Skeletons'
import { EpisodeLegend } from './EpisodeLegend'
import { SeasonCard } from './SeasonCard'

export function SeriesDetailView() {
  const { tvdbId: raw } = useParams()
  const navigate = useNavigate()
  const tvdbId = Number(raw)
  const series = useSeries(tvdbId)

  if (!Number.isFinite(tvdbId)) return <NotFound raw={raw} onBack={() => navigate('/')} />

  if (series.isPending) return <DetailSkeleton />

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

  return (
    <div className="space-y-6">
      <Link to="/" className="inline-block text-sm text-slate-400 hover:text-slate-200">
        ← Back to catalog
      </Link>

      <DetailHeaderCard
        type="series"
        displayName={data.displayName}
        year={data.year}
        instances={data.instances}
        sizeOnDiskBytes={data.sizeOnDiskBytes}
        fileCount={data.fileCount}
        playback={playback}
      />

      <section className="space-y-3">
        <div className="flex flex-wrap items-center gap-x-4 gap-y-2">
          <h2 className="text-sm font-semibold text-slate-100">Seasons</h2>
          {data.seasons.length > 0 && <EpisodeLegend playbackAvailable={playback !== null} />}
        </div>
        {data.seasons.length === 0 ? (
          <p className="text-sm text-slate-400">No season file data.</p>
        ) : (
          data.seasons.map((season) => (
            <SeasonCard key={season.seasonNumber} season={season} />
          ))
        )}
        {data.unattributedPlayCount !== null && data.unattributedPlayCount > 0 && (
          <p className="text-xs text-slate-500">
            {data.unattributedPlayCount.toLocaleString()}{' '}
            {data.unattributedPlayCount === 1 ? "play isn't" : "plays aren't"} shown per episode
            (synced before episode tracking, missing episode info, or specials).
          </p>
        )}
      </section>

      <InstanceTable
        extraHeader="Seasons"
        rows={data.instanceDetails.map((detail) => ({
          instance: detail.instance,
          sizeOnDiskBytes: detail.sizeOnDiskBytes,
          fileCount: detail.fileCount,
          extra: detail.seasonNumbers.join(', '),
        }))}
      />
    </div>
  )
}
