import { Link, useNavigate, useParams } from 'react-router'
import { useArtist } from '../api/queries'
import { ApiError } from '../api/http'
import type { ArtistDetails } from '../api/types'
import type { LinkTarget } from '../lib/deepLink'
import { Button } from '../components/Button'
import { DetailHeaderCard } from '../components/DetailHeaderCard'
import { EmptyState } from '../components/EmptyState'
import { InstanceTable } from '../components/InstanceTable'
import { DetailSkeleton } from '../components/Skeletons'
import { formatBytes } from '../lib/format'

export function ArtistDetailView() {
  // react-router decodes the param, so the id arrives in its stored form.
  const { musicBrainzId: raw } = useParams()
  const navigate = useNavigate()
  const artist = useArtist(raw ?? '')

  if (!raw) return <NotFound raw={raw} onBack={() => navigate('/')} />

  if (artist.isPending) return <DetailSkeleton />

  if (artist.isError) {
    if (artist.error instanceof ApiError && artist.error.status === 404) {
      return <NotFound raw={raw} onBack={() => navigate('/')} />
    }
    return (
      <EmptyState
        title="Couldn't load this artist"
        description={artist.error instanceof Error ? artist.error.message : 'Unknown error'}
        action={<Button onClick={() => artist.refetch()}>Retry</Button>}
      />
    )
  }

  return <ArtistDetail data={artist.data} />
}

function NotFound({ raw, onBack }: { raw?: string; onBack: () => void }) {
  return (
    <EmptyState
      title="Artist not found"
      description={`No artist with MusicBrainz id ${raw ?? ''}.`}
      action={<Button onClick={onBack}>Back to catalog</Button>}
    />
  )
}

function ArtistDetail({ data }: { data: ArtistDetails }) {
  const target: LinkTarget = { contentType: 'artist', musicBrainzId: data.musicBrainzId }
  return (
    <div className="space-y-6">
      <Link to="/" className="inline-block text-sm text-slate-400 hover:text-slate-200">
        ← Back to catalog
      </Link>

      <DetailHeaderCard
        type="artist"
        displayName={data.displayName}
        instances={data.instances}
        target={target}
        sizeOnDiskBytes={data.sizeOnDiskBytes}
        fileCount={data.fileCount}
        playback={data.playback}
      />

      <section className="rounded-lg border border-slate-800 bg-slate-900/40">
        <header className="flex items-center justify-between border-b border-slate-800 px-4 py-3">
          <h2 className="text-sm font-semibold text-slate-100">Albums</h2>
        </header>
        {data.albums.length === 0 ? (
          <p className="p-4 text-sm text-slate-400">No album file data.</p>
        ) : (
          <div className="p-4">
            <table className="w-full border-separate border-spacing-0 text-sm">
              <thead>
                <tr>
                  <th className="border-b border-slate-800 px-3 py-2 text-left text-xs font-semibold uppercase tracking-wide text-slate-400">
                    Title
                  </th>
                  <th className="border-b border-slate-800 px-3 py-2 text-right text-xs font-semibold uppercase tracking-wide text-slate-400">
                    Size
                  </th>
                  <th className="border-b border-slate-800 px-3 py-2 text-right text-xs font-semibold uppercase tracking-wide text-slate-400">
                    Files
                  </th>
                </tr>
              </thead>
              <tbody>
                {data.albums.map((album) => (
                  <tr key={album.musicBrainzId} className="hover:bg-slate-800/30">
                    <td className="border-b border-slate-800/60 px-3 py-1.5 text-slate-200">
                      {album.title || (
                        // Pre-migration snapshot rows lack titles until the
                        // next Lidarr sync fills them.
                        <span className="italic text-slate-500">Unknown album</span>
                      )}
                    </td>
                    <td className="border-b border-slate-800/60 px-3 py-1.5 text-right tabular-nums">
                      {formatBytes(album.sizeOnDiskBytes)}
                    </td>
                    <td className="border-b border-slate-800/60 px-3 py-1.5 text-right tabular-nums">
                      {album.fileCount.toLocaleString()}
                    </td>
                  </tr>
                ))}
              </tbody>
            </table>
          </div>
        )}
      </section>

      <InstanceTable
        extraHeader="Albums"
        target={target}
        rows={data.instanceDetails.map((detail) => ({
          instance: detail.instance,
          sizeOnDiskBytes: detail.sizeOnDiskBytes,
          fileCount: detail.fileCount,
          extra: detail.albumCount.toLocaleString(),
        }))}
      />
    </div>
  )
}
