import type { ContentType, InstanceReference, PlaybackMetrics } from '../api/types'
import { InstanceChips } from './InstanceChips'
import { StatCard } from './StatCard'
import { TypeBadge } from './TypeBadge'
import { absoluteTime, formatBytes, formatDuration, relativeTime } from '../lib/format'

interface DetailHeaderCardProps {
  type: ContentType
  displayName: string
  // Omitted for artists — Lidarr doesn't model a year.
  year?: number
  instances: InstanceReference[]
  sizeOnDiskBytes: number
  fileCount: number
  playback: PlaybackMetrics | null
}

export function DetailHeaderCard({
  type,
  displayName,
  year,
  instances,
  sizeOnDiskBytes,
  fileCount,
  playback,
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
      <div className="grid grid-cols-2 gap-4 p-4 sm:grid-cols-4">
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
  )
}
