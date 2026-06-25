import { PLAYBACK_NOT_CONFIGURED, type PlaybackSyncState } from '../api/client'
import { useStartPlaybackSync } from '../api/mutations'
import { usePlaybackSyncStatus } from '../api/queries'
import type { PlaybackSyncRun } from '../api/types'
import { Button } from '../components/Button'
import { StatusChip } from '../components/StatusChip'
import { absoluteTime, elapsedSeconds, formatDuration, relativeTime } from '../lib/format'

export function PlaybackSyncPanel() {
  const playback = usePlaybackSyncStatus()
  const start = useStartPlaybackSync()

  // Hide entirely while loading or when no playback source is configured (404).
  const data: PlaybackSyncState | undefined = playback.data
  if (data === undefined || data === PLAYBACK_NOT_CONFIGURED) return null

  const run: PlaybackSyncRun | null = data
  const running = run?.status === 'running'

  return (
    <section className="rounded-lg border border-slate-800 bg-slate-900/40">
      <header className="flex items-center justify-between border-b border-slate-800 px-4 py-3">
        <h2 className="text-sm font-semibold text-slate-100">Playback sync</h2>
        <Button onClick={() => start.mutate()} disabled={running || start.isPending}>
          {running ? 'Syncing…' : start.isPending ? 'Starting…' : 'Run playback sync now'}
        </Button>
      </header>
      <div className="p-4 text-sm">
        {!run ? (
          <p className="text-slate-400">No playback sync has run yet.</p>
        ) : (
          <div className="space-y-2">
            <div className="flex flex-wrap items-center gap-x-4 gap-y-1">
              <StatusChip status={run.status} />
              <span className="text-slate-400">
                Started{' '}
                <span className="text-slate-200" title={absoluteTime(run.startedAt)}>
                  {relativeTime(run.startedAt)}
                </span>
              </span>
              <span className="text-slate-400">
                Duration:{' '}
                <span className="text-slate-200">
                  {run.completedAt
                    ? formatDuration(elapsedSeconds(run.startedAt, run.completedAt))
                    : 'in progress'}
                </span>
              </span>
            </div>
            <div className="flex gap-4 text-slate-400">
              <span>
                Matched{' '}
                <span className="tabular-nums text-slate-200">
                  {run.matchedHistoryRows.toLocaleString()}
                </span>
              </span>
              <span
                title={run.unmatchedHistoryRows > 0 ? 'Unmatched rows make a run partial' : undefined}
              >
                Unmatched{' '}
                <span
                  className={`tabular-nums ${
                    run.unmatchedHistoryRows > 0 ? 'text-amber-300' : 'text-slate-200'
                  }`}
                >
                  {run.unmatchedHistoryRows.toLocaleString()}
                </span>
              </span>
            </div>
            {run.error && <p className="text-red-300">{run.error}</p>}
          </div>
        )}
        {start.isError && (
          <p className="mt-3 text-red-300">
            {start.error instanceof Error ? start.error.message : 'Failed to start playback sync'}
          </p>
        )}
      </div>
    </section>
  )
}
