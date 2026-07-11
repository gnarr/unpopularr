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

// One calendar day of movie playback (UTC) — the finest grain the backend
// returns. The chart re-buckets these to the resolution the user picks. Only
// days that had playback are present; the chart fills the gaps.
export interface DailyPlayback {
  // `YYYY-MM-DD` (UTC).
  date: string
  playCount: number
  playDurationSeconds: number
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

// Per-series detail (GET /api/v1/series/{tvdbId}). Mirrors the backend
// `SeriesDetails` serde struct; the season/instance breakdowns are data the flat
// catalog list discards.
export interface SeriesEpisodeDetail {
  episodeNumber: number
  title: string
  airDateUtc: string | null
  hasFile: boolean
  sizeOnDiskBytes: number
  playback: PlaybackMetrics | null
}

export interface SeriesSeasonDetail {
  seasonNumber: number
  fileCount: number
  // Distinct episodes known to Sonarr, aired or not. 0 when the snapshot
  // predates episode collection (run a sync to populate).
  episodeCount: number
  episodesWithFiles: number
  sizeOnDiskBytes: number
  playback: PlaybackMetrics | null
  episodes: SeriesEpisodeDetail[]
}

export interface SeriesInstanceDetail {
  instance: InstanceReference
  sizeOnDiskBytes: number
  fileCount: number
  seasonNumbers: number[]
}

export interface SeriesDetails {
  displayName: string
  tvdbId: number
  year: number
  sizeOnDiskBytes: number
  fileCount: number
  instances: InstanceReference[]
  seasons: SeriesSeasonDetail[]
  instanceDetails: SeriesInstanceDetail[]
  playback: PlaybackMetrics | null
  // Series-level plays with no episode cell to land on (legacy aggregates,
  // events without positions, specials). Null when playback is unavailable.
  unattributedPlayCount: number | null
}

// Per-movie detail (GET /api/v1/movies/{tmdbId}). Mirrors the backend
// `MovieDetails` serde struct.
export interface MovieInstanceDetail {
  instance: InstanceReference
  sizeOnDiskBytes: number
  fileCount: number
}

export interface MovieDetails {
  displayName: string
  tmdbId: number
  year: number
  sizeOnDiskBytes: number
  fileCount: number
  instances: InstanceReference[]
  instanceDetails: MovieInstanceDetail[]
  playback: PlaybackMetrics | null
  // Earliest Radarr "added" date across instances — the plot's left edge.
  // `null` until a re-sync populates it (fall back to the first played day).
  availableAt: string | null
  // Per-day playback totals, ascending by day.
  dailyPlayback: DailyPlayback[]
}

// Per-artist detail (GET /api/v1/artists/{musicBrainzId}). Mirrors the backend
// `ArtistDetails` serde struct. Artists have no year — Lidarr doesn't model one.
export interface ArtistAlbumDetail {
  musicBrainzId: string
  // '' for snapshots synced before the title column existed; the next Lidarr
  // sync fills it.
  title: string
  sizeOnDiskBytes: number
  fileCount: number
}

export interface ArtistInstanceDetail {
  instance: InstanceReference
  sizeOnDiskBytes: number
  fileCount: number
  albumCount: number
}

export interface ArtistDetails {
  displayName: string
  musicBrainzId: string
  sizeOnDiskBytes: number
  fileCount: number
  instances: InstanceReference[]
  albums: ArtistAlbumDetail[]
  instanceDetails: ArtistInstanceDetail[]
  playback: PlaybackMetrics | null
}

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
