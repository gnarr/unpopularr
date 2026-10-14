import type { ContentType, InstanceReference, PlaybackMetrics } from '../api/types'
import { InstanceChips } from './InstanceChips'
import { StatCard } from './StatCard'
import { TypeBadge } from './TypeBadge'
import { absoluteTime, formatBytes, formatDuration, relativeTime } from '../lib/format'
import { POPULARITY_HINT, RECLAIM_HINT } from '../lib/content'

interface DetailHeaderCardProps {
  type: ContentType
  displayName: string
  // Omitted for artists — Lidarr doesn't model a year.
  year?: number
  instances: InstanceReference[]
  sizeOnDiskBytes: number
  fileCount: number
  // Oldest acquisition date across instances; null when nothing reports one.
  addedAt: string | null
  playback: PlaybackMetrics | null
  // Null when playback is unavailable, like `playback`.
  popularityScore: number | null
  // Null when any input is unknown, not only playback.
  reclaimScore: number | null
}

export function DetailHeaderCard({
  type,
  displayName,
  year,
  instances,
  sizeOnDiskBytes,
  fileCount,
  addedAt,
  playback,
  popularityScore,
  reclaimScore,
}: DetailHeaderCardProps) {
  const neverPlayed = playback !== null && playback.playCount === 0

  return (
    <section className="rounded-lg border border-slate-800 bg-slate-900/40">
      <header className="flex flex-wrap items-center gap-3 border-b border-slate-800 px-4 py-3">
        <TypeBadge type={type} />
        <h1 className="text-lg font-semibold text-slate-100">
          {displayName} {year !== undefined && <span className="text-slate-500">({year})</span>}
        </h1>
        <div className="ml-auto">
          <InstanceChips instances={instances} />
        </div>
      </header>
      <div className="grid grid-cols-2 gap-4 p-4 sm:grid-cols-3 lg:grid-cols-6">
        <StatCard label="Total size" value={formatBytes(sizeOnDiskBytes)} />
        <StatCard label="Files" value={fileCount.toLocaleString()} />
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
        {popularityScore !== null && (
          <StatCard label="Popularity" value={`${popularityScore} / 100`} hint={POPULARITY_HINT} />
        )}
        {reclaimScore !== null && (
          <StatCard label="Reclaim" value={`${reclaimScore} / 100`} hint={RECLAIM_HINT} />
        )}
      </div>
      {(addedAt !== null || playback?.lastPlayedAt) && (
        <div className="flex flex-wrap gap-x-4 border-t border-slate-800 px-4 py-2 text-sm text-slate-400">
          {addedAt !== null && (
            <span>
              Added{' '}
              <span className="text-slate-200" title={absoluteTime(addedAt)}>
                {relativeTime(addedAt)}
              </span>
            </span>
          )}
          {playback?.lastPlayedAt && (
            <span>
              Last played{' '}
              <span className="text-slate-200" title={absoluteTime(playback.lastPlayedAt)}>
                {relativeTime(playback.lastPlayedAt)}
              </span>
            </span>
          )}
        </div>
      )}
    </section>
  )
}
