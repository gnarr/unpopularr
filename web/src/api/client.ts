import { ApiError, request } from './http'
import type { ContentItem, PlaybackSyncRun, SyncRun } from './types'

export const PLAYBACK_NOT_CONFIGURED = 'not-configured' as const
export type PlaybackSyncState = PlaybackSyncRun | null | typeof PLAYBACK_NOT_CONFIGURED

export function getContent(): Promise<ContentItem[]> {
  return request<ContentItem[]>('/api/v1/content').then((data) => data ?? [])
}

export function getSyncStatus(): Promise<SyncRun | null> {
  return request<SyncRun>('/api/v1/sync')
}

// A missing playback feature returns 404; surface that as an expected state
// rather than an error so the UI can simply hide playback controls.
export async function getPlaybackSyncStatus(): Promise<PlaybackSyncState> {
  try {
    return await request<PlaybackSyncRun>('/api/v1/playback/sync')
  } catch (error) {
    if (error instanceof ApiError && error.status === 404) return PLAYBACK_NOT_CONFIGURED
    throw error
  }
}

export function startSync(): Promise<SyncRun> {
  return startRun<SyncRun>('/api/v1/sync')
}

export function startPlaybackSync(): Promise<PlaybackSyncRun> {
  return startRun<PlaybackSyncRun>('/api/v1/playback/sync')
}

function isRun(body: unknown): body is { status: string } {
  return typeof body === 'object' && body !== null && 'status' in body
}

// 202 Accepted and 409 Conflict both may carry the run as JSON. We treat the
// active run returned by a 409 as the current state, not an error. 409 with only
// `{ error }`, plus 404/5xx, reject so callers can surface the message.
async function startRun<T>(path: string): Promise<T> {
  const response = await fetch(path, {
    method: 'POST',
    headers: { Accept: 'application/json' },
  })
  const body: unknown = await response.json().catch(() => null)

  if ((response.status === 202 || response.status === 409) && isRun(body)) {
    return body as T
  }

  const message =
    typeof (body as { error?: unknown })?.error === 'string'
      ? (body as { error: string }).error
      : `request failed (${response.status})`
  throw new ApiError(response.status, message)
}
