# Unpopularr

Unpopularr is a self-hosted media analytics backend. This first version imports
Sonarr, Radarr, and Lidarr libraries into SQLite and exposes a combined view of
content that has files.

Multiple instances of each application are supported. Movies are combined by
TMDB ID, series by TVDB ID, and artists by MusicBrainz ID. Disk sizes and file
counts are summed across instances; season and album counts are deduplicated.

## Run

Requirements:

- Rust 1.94 or newer
- Sonarr v3 API, Radarr v3 API, and/or Lidarr v1 API access

Copy `config.example.toml` to `config.toml`, edit the instances, and export each
referenced API-key environment variable:

```sh
export RADARR_HD_API_KEY='...'
export RADARR_4K_API_KEY='...'
export SONARR_HD_API_KEY='...'
export LIDARR_API_KEY='...'
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
    "year": 2024
  }
]
```

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
