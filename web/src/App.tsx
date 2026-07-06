import { useEffect, useRef } from 'react'
import { useQueryClient } from '@tanstack/react-query'
import { Navigate, Outlet, Route, Routes } from 'react-router'
import { Header } from './components/Header'
import { CatalogView } from './catalog/CatalogView'
import { ActivityView } from './activity/ActivityView'
import { SeriesDetailView } from './series/SeriesDetailView'
import { usePlaybackSyncStatus, useSyncStatus } from './api/queries'
import { PLAYBACK_NOT_CONFIGURED } from './api/client'
import { queryKeys } from './api/queryKeys'
import type { SyncStatus } from './api/types'

export default function App() {
  return (
    <Routes>
      <Route element={<Layout />}>
        <Route index element={<CatalogView />} />
        <Route path="activity" element={<ActivityView />} />
        <Route path="series/:tvdbId" element={<SeriesDetailView />} />
        <Route path="*" element={<Navigate to="/" replace />} />
      </Route>
    </Routes>
  )
}

// The shell is a layout route so it stays mounted across navigation — that keeps
// useRefreshContentAfterSync() running on every page, not just the catalog.
function Layout() {
  useRefreshContentAfterSync()
  return (
    <div className="min-h-full">
      <Header />
      <main className="mx-auto max-w-screen-2xl px-4 py-6 sm:px-6">
        <Outlet />
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

  // A completed sync lands fresh snapshots/playback, so refresh both the catalog
  // list and any open series detail page (cached under the ['series', id] prefix).
  const refresh = () => {
    queryClient.invalidateQueries({ queryKey: queryKeys.content })
    queryClient.invalidateQueries({ queryKey: queryKeys.seriesAll })
  }
  useTerminalTransition(syncStatus, refresh)
  useTerminalTransition(playbackStatus, refresh)
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
