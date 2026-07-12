import { ApiError, request } from './http'
import type {
  ArtistDetails,
  ContentItem,
  InstanceLink,
  MovieDetails,
  PlaybackSyncRun,
  SeriesDetails,
  SyncRun,
} from './types'

export const PLAYBACK_NOT_CONFIGURED = 'not-configured' as const
export type PlaybackSyncState = PlaybackSyncRun | null | typeof PLAYBACK_NOT_CONFIGURED

export function getContent(): Promise<ContentItem[]> {
  return request<ContentItem[]>('/api/v1/content').then((data) => data ?? [])
}

// Browser-facing deep-link metadata for every configured instance.
export function getInstances(): Promise<InstanceLink[]> {
  return request<InstanceLink[]>('/api/v1/instances').then((data) => data ?? [])
}

// A missing item 404s; unlike playback that ApiError is surfaced so the detail
// views can render a "not found" state.
export async function getSeries(tvdbId: number): Promise<SeriesDetails> {
  const data = await request<SeriesDetails>(`/api/v1/series/${tvdbId}`)
  if (data === null) throw new ApiError(502, 'empty series response')
  return data
}

export async function getMovie(tmdbId: number): Promise<MovieDetails> {
  const data = await request<MovieDetails>(`/api/v1/movies/${tmdbId}`)
  if (data === null) throw new ApiError(502, 'empty movie response')
  return data
}

export async function getArtist(musicBrainzId: string): Promise<ArtistDetails> {
  const data = await request<ArtistDetails>(
    `/api/v1/artists/${encodeURIComponent(musicBrainzId)}`,
  )
  if (data === null) throw new ApiError(502, 'empty artist response')
  return data
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
