import { usePlaybackSyncStatus, useSyncStatus } from '../api/queries'
import { PLAYBACK_NOT_CONFIGURED } from '../api/client'

// Always-visible indicator that a sync (library or playback) is in progress.
export function SyncBadge() {
  const sync = useSyncStatus()
  const playback = usePlaybackSyncStatus()

  const playbackData = playback.data
  const running =
    sync.data?.status === 'running' ||
    (playbackData != null &&
      playbackData !== PLAYBACK_NOT_CONFIGURED &&
      playbackData.status === 'running')

  if (!running) return null

  return (
    <span className="inline-flex items-center gap-2 rounded-full bg-blue-500/15 px-2.5 py-1 text-xs font-medium text-blue-300 ring-1 ring-inset ring-blue-500/30">
      <span className="h-2 w-2 animate-pulse rounded-full bg-blue-400" />
      Syncing…
    </span>
  )
}
