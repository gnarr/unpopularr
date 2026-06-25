import { useMutation, useQueryClient } from '@tanstack/react-query'
import { startPlaybackSync, startSync } from './client'
import { queryKeys } from './queryKeys'

export function useStartSync() {
  const queryClient = useQueryClient()
  return useMutation({
    mutationFn: startSync,
    // Adopt the returned run immediately so polling begins without waiting for
    // the next status refetch.
    onSuccess: (run) => queryClient.setQueryData(queryKeys.sync, run),
    onSettled: () => queryClient.invalidateQueries({ queryKey: queryKeys.sync }),
  })
}

export function useStartPlaybackSync() {
  const queryClient = useQueryClient()
  return useMutation({
    mutationFn: startPlaybackSync,
    onSuccess: (run) => queryClient.setQueryData(queryKeys.playbackSync, run),
    onSettled: () => queryClient.invalidateQueries({ queryKey: queryKeys.playbackSync }),
  })
}
