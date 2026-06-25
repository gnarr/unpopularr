import { useEffect, useRef, useState } from 'react'
import { useQueryClient } from '@tanstack/react-query'
import { Header } from './components/Header'
import { CatalogView } from './catalog/CatalogView'
import { ActivityView } from './activity/ActivityView'
import { usePlaybackSyncStatus, useSyncStatus } from './api/queries'
import { PLAYBACK_NOT_CONFIGURED } from './api/client'
import { queryKeys } from './api/queryKeys'
import type { SyncStatus } from './api/types'

export type View = 'catalog' | 'activity'

export default function App() {
  const [view, setView] = useState<View>('catalog')
  useRefreshContentAfterSync()

  return (
    <div className="min-h-full">
      <Header view={view} onChange={setView} />
      <main className="mx-auto max-w-screen-2xl px-4 py-6 sm:px-6">
        {view === 'catalog' ? (
          <CatalogView onGoToActivity={() => setView('activity')} />
        ) : (
          <ActivityView />
        )}
      </main>
    </div>
  )
}

// Refresh the catalog once a sync transitions from running to a terminal state,
// since fresh snapshots / playback data have just landed.
function useRefreshContentAfterSync() {
  const queryClient = useQueryClient()
  const sync = useSyncStatus()
  const playback = usePlaybackSyncStatus()

  const syncStatus = sync.data?.status
  const playbackData = playback.data
  const playbackStatus =
    playbackData != null && playbackData !== PLAYBACK_NOT_CONFIGURED
      ? playbackData.status
      : undefined

  useTerminalTransition(syncStatus, () =>
    queryClient.invalidateQueries({ queryKey: queryKeys.content }),
  )
  useTerminalTransition(playbackStatus, () =>
    queryClient.invalidateQueries({ queryKey: queryKeys.content }),
  )
}

function useTerminalTransition(status: SyncStatus | undefined, onTerminal: () => void) {
  const previous = useRef<SyncStatus | undefined>(undefined)
  useEffect(() => {
    if (previous.current === 'running' && status !== undefined && status !== 'running') {
      onTerminal()
    }
    previous.current = status
    // onTerminal is recreated each render; we intentionally key only on status.
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [status])
}
