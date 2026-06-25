// Mirrors the backend's serde JSON contract (camelCase). ISO timestamps are kept
// as strings and parsed only at the formatting boundary.

export interface InstanceReference {
  id: string
  name: string
  lastSuccessfulSyncAt: string
}

export interface PlaybackMetrics {
  playCount: number
  playDurationSeconds: number
  lastPlayedAt: string | null
}

interface ContentBase {
  displayName: string
  sizeOnDiskBytes: number
  fileCount: number
  instances: InstanceReference[]
  // `null` means playback is unavailable (no provider configured, or no sync yet).
  // When available, never-played content has playCount 0 and lastPlayedAt null.
  playback: PlaybackMetrics | null
}

export interface MovieItem extends ContentBase {
  contentType: 'movie'
  tmdbId: number
  year: number
}

export interface SeriesItem extends ContentBase {
  contentType: 'series'
  tvdbId: number
  year: number
  seasonsWithFiles: number
}

export interface ArtistItem extends ContentBase {
  contentType: 'artist'
  musicBrainzId: string
  albumsWithFiles: number
}

export type ContentItem = MovieItem | SeriesItem | ArtistItem
export type ContentType = ContentItem['contentType']

export type SyncTrigger = 'startup' | 'scheduled' | 'manual'
export type SyncStatus = 'running' | 'succeeded' | 'partial' | 'failed'
export type InstanceKind = 'sonarr' | 'radarr' | 'lidarr'

export interface InstanceSyncResult {
  id: string
  name: string
  kind: InstanceKind
  status: SyncStatus
  importedItems: number
  error: string | null
  startedAt: string
  completedAt: string | null
}

export interface SyncRun {
  id: number
  trigger: SyncTrigger
  status: SyncStatus
  startedAt: string
  completedAt: string | null
  importedItems: number
  instances: InstanceSyncResult[]
}

export interface PlaybackSyncRun {
  id: number
  sourceId: string
  trigger: SyncTrigger
  status: SyncStatus
  startedAt: string
  completedAt: string | null
  matchedHistoryRows: number
  unmatchedHistoryRows: number
  error: string | null
}
