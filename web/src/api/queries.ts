import { useQuery } from '@tanstack/react-query'
import {
  PLAYBACK_NOT_CONFIGURED,
  getContent,
  getPlaybackSyncStatus,
  getSeries,
  getSyncStatus,
} from './client'
import { ApiError } from './http'
import { queryKeys } from './queryKeys'

const POLL_INTERVAL_MS = 2000

export function useContent() {
  return useQuery({
    queryKey: queryKeys.content,
    queryFn: getContent,
    staleTime: 60_000,
  })
}

export function useSeries(tvdbId: number) {
  return useQuery({
    queryKey: queryKeys.series(tvdbId),
    queryFn: () => getSeries(tvdbId),
    staleTime: 60_000,
    enabled: Number.isFinite(tvdbId),
    // A 404 is a definitive "no such series" — don't burn the default retry on it.
    retry: (failureCount, error) =>
      error instanceof ApiError && error.status === 404 ? false : failureCount < 1,
  })
}

export function useSyncStatus() {
  return useQuery({
    queryKey: queryKeys.sync,
    queryFn: getSyncStatus,
    // Poll only while a sync is running; stop on any terminal status.
    refetchInterval: (query) =>
      query.state.data?.status === 'running' ? POLL_INTERVAL_MS : false,
    refetchOnWindowFocus: true,
  })
}

export function usePlaybackSyncStatus() {
  return useQuery({
    queryKey: queryKeys.playbackSync,
    queryFn: getPlaybackSyncStatus,
    retry: false,
    refetchInterval: (query) => {
      const data = query.state.data
      const running =
        data != null && data !== PLAYBACK_NOT_CONFIGURED && data.status === 'running'
      return running ? POLL_INTERVAL_MS : false
    },
    refetchOnWindowFocus: true,
  })
}
