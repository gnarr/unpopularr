import { Link, useNavigate, useParams } from 'react-router'
import { useMovie } from '../api/queries'
import { ApiError } from '../api/http'
import type { MovieDetails } from '../api/types'
import { Button } from '../components/Button'
import { DetailHeaderCard } from '../components/DetailHeaderCard'
import { EmptyState } from '../components/EmptyState'
import { InstanceTable } from '../components/InstanceTable'
import { DetailSkeleton } from '../components/Skeletons'
import { MoviePlaybackChart } from './MoviePlaybackChart'

export function MovieDetailView() {
  const { tmdbId: raw } = useParams()
  const navigate = useNavigate()
  const tmdbId = Number(raw)
  const movie = useMovie(tmdbId)

  if (!Number.isFinite(tmdbId)) return <NotFound raw={raw} onBack={() => navigate('/')} />

  if (movie.isPending) return <DetailSkeleton />

  if (movie.isError) {
    if (movie.error instanceof ApiError && movie.error.status === 404) {
      return <NotFound raw={raw} onBack={() => navigate('/')} />
    }
    return (
      <EmptyState
        title="Couldn't load this movie"
        description={movie.error instanceof Error ? movie.error.message : 'Unknown error'}
        action={<Button onClick={() => movie.refetch()}>Retry</Button>}
      />
    )
  }

  return <MovieDetail data={movie.data} />
}

function NotFound({ raw, onBack }: { raw?: string; onBack: () => void }) {
  return (
    <EmptyState
      title="Movie not found"
      description={`No movie with TMDB id ${raw ?? ''}.`}
      action={<Button onClick={onBack}>Back to catalog</Button>}
    />
  )
}

function MovieDetail({ data }: { data: MovieDetails }) {
  return (
    <div className="space-y-6">
      <Link to="/" className="inline-block text-sm text-slate-400 hover:text-slate-200">
        ← Back to catalog
      </Link>

      <DetailHeaderCard
        type="movie"
        displayName={data.displayName}
        year={data.year}
        instances={data.instances}
        sizeOnDiskBytes={data.sizeOnDiskBytes}
        fileCount={data.fileCount}
        playback={data.playback}
      />

      {data.playback !== null && data.dailyPlayback.length > 0 && (
        <section className="rounded-lg border border-slate-800 bg-slate-900/40">
          <header className="border-b border-slate-800 px-4 py-3">
            <h2 className="text-sm font-semibold text-slate-100">Minutes played</h2>
          </header>
          <div className="p-4">
            <MoviePlaybackChart dailyPlayback={data.dailyPlayback} availableAt={data.availableAt} />
          </div>
        </section>
      )}

      <InstanceTable
        rows={data.instanceDetails.map((detail) => ({
          instance: detail.instance,
          sizeOnDiskBytes: detail.sizeOnDiskBytes,
          fileCount: detail.fileCount,
        }))}
      />
    </div>
  )
}
