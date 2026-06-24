# Unpopularr

Unpopularr is a self-hosted media analytics backend. It imports Sonarr, Radarr,
and Lidarr libraries into SQLite and exposes a combined view of content that
has files. It can optionally import aggregate playback analytics from
Tautulli.

Multiple instances of each application are supported. Movies are combined by
TMDB ID, series by TVDB ID, and artists by MusicBrainz ID. Disk sizes and file
counts are summed across instances; season and album counts are deduplicated.

## Run

Requirements:

- Rust 1.94 or newer
- Sonarr v3 API, Radarr v3 API, and/or Lidarr v1 API access
- Optional Tautulli API access for Plex playback analytics

Copy `config.example.toml` to `config.toml`, edit the instances, and export each
referenced API-key environment variable:

```sh
export RADARR_HD_API_KEY='...'
export RADARR_4K_API_KEY='...'
export SONARR_HD_API_KEY='...'
export LIDARR_API_KEY='...'
export TAUTULLI_API_KEY='...'
rtk cargo run
```

Set `UNPOPULARR_CONFIG` to use a different configuration path. Logging is
controlled with `RUST_LOG`.

The service has no built-in authentication. Bind it only to a trusted network,
or place it behind an authenticating reverse proxy.

Instance `base_url` values must reach the Arr API directly. If an
authentication proxy such as Authelia protects the instance, either use an
internal URL that bypasses it or configure the proxy to bypass authentication
for that instance's `/api/` routes. Unpopularr deliberately does not follow
redirects so API keys cannot be forwarded to another host.

The optional `[playback]` configuration supports one Tautulli source. Its
`base_url` must also reach the API directly. Unpopularr periodically reads all
history retained by Tautulli and stores only aggregate play count, duration,
and last-played time by movie, series, or artist. It does not retain users,
devices, IP addresses, or individual playback sessions.

Tautulli can only report history recorded while it was installed and running.
It cannot retroactively import older Plex playback history.

## API

### Get content

```http
GET /api/v1/content
```

Returns a JSON array sorted by content type and case-insensitive display name.
Only aggregated entries with at least one file are returned.

```json
[
  {
    "contentType": "movie",
    "displayName": "Example",
    "sizeOnDiskBytes": 123456,
    "fileCount": 2,
    "instances": [
      {
        "id": "radarr-hd",
        "name": "Radarr HD",
        "lastSuccessfulSyncAt": "2026-06-24T12:00:00Z"
      }
    ],
    "tmdbId": 123,
    "year": 2024,
    "playback": {
      "playCount": 4,
      "playDurationSeconds": 7200,
      "lastPlayedAt": "2026-06-24T12:00:00Z"
    }
  }
]
```

`playback` is `null` when playback collection is disabled or no playback sync
has completed successfully. After a successful sync, content with no matched
history has zero counts and a `null` `lastPlayedAt`.

### Start a sync

```http
POST /api/v1/sync
```

Returns `202 Accepted` with the running sync record. If a sync is already
running, returns `409 Conflict` with that record when available. If the request
races with creation of the sync record, the response is
`{"error":"a sync is already running"}`.

### Get sync status

```http
GET /api/v1/sync
```

Returns the active sync or most recently completed sync. Returns `204 No
Content` if no sync has been created.

Successful instance snapshots are replaced atomically. If an instance fails,
its previous snapshot remains available and the overall run is marked
`partial` or `failed`.

### Start a playback sync

```http
POST /api/v1/playback/sync
```

Returns `202 Accepted` with the running playback sync. If one is already
running, returns `409 Conflict`.

### Get playback sync status

```http
GET /api/v1/playback/sync
```

Returns the active or most recent playback sync, including matched and
unmatched history-row counts. Returns `204 No Content` before the first run.
These routes are absent when `[playback]` is not configured.

Playback snapshots are replaced atomically. Source, parsing, and persistence
failures preserve the last successful snapshot. A run is `partial` when some
relevant history cannot be matched through TMDB, TVDB, or MusicBrainz IDs, and
`failed` when relevant history exists but none of it can be matched.

## Development

All shell commands on this machine must be run through RTK:

```sh
rtk cargo fix --allow-dirty --allow-staged
rtk cargo fmt
rtk cargo clippy --all-targets --all-features -- -D warnings
rtk cargo test --all-targets --all-features
```

Database migrations are embedded from `migrations/` and applied at startup.
Add a new migration instead of modifying a migration that has shipped.
