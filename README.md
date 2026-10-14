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
cargo run
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
history retained by Tautulli and stores each playback session's timestamp,
duration, position within a series, and watching user (Tautulli user id and
display name), keyed by movie, series, or artist. It does not retain devices,
IP addresses, or what was played beyond those fields.

Tautulli can only report history recorded while it was installed and running.
It cannot retroactively import older Plex playback history.

### Media age

Every item carries an age: how long it has been in your library, taken as the
oldest of every date the sources report — Radarr's `added` and its movie file's
`dateAdded`, Sonarr's `series.added` and each episode file's `dateAdded`, and
Lidarr's artist and album dates. A series ages from its oldest episode file, an
artist from its oldest album. Release and air dates are deliberately excluded:
they measure the content's age, not the library's.

The optional `[media_probe]` section adds one more source, the files themselves.
The *arr apps only know when *they* learned about a file, so rebuilding a Sonarr
database makes every series look days old; reading the file's own timestamps
recovers the real date. This is off by default because it needs filesystem
access unpopularr otherwise never has.

Mount your media **read-only** — the probe only stats files, it never opens or
writes them:

```sh
docker run \
  -v /srv/appdata/unpopularr:/app/data \
  -v /srv/media:/media:ro \
  ghcr.io/gnarr/unpopularr
```

or in compose, `- /srv/media:/media:ro`.

Then map the paths. The `from` prefix is the path as the *arr app reports it
(what its own UI shows under a movie or series), not what unpopularr sees:

```toml
[media_probe]
path_mappings = [
  { from = "/data/media/movies", to = "/media/movies" },
  { from = "/data/media/tv", to = "/media/tv" },
]
```

Omit `path_mappings` entirely when unpopularr already sees media at the same
paths the *arr apps use. The longest matching prefix wins, so declaration order
does not matter.

Two things to know:

- The image runs as uid/gid 10001, and stat requires execute permission on
  every parent directory. Either make the media tree traversable by that uid or
  run with `--user $(id -u):$(id -g)`.
- Music ages are folder-granular: Lidarr artists are probed at the folder, not
  per track file, which would cost an extra API request per artist.

Anything unreadable simply contributes no hint; the probe never fails a sync.
It logs one line per instance per sync, and warns when no path matched a
mapping rule or no file could be read — the usual sign of a wrong prefix or a
missing mount.

### Popularity score

With playback configured, every movie, series, and artist carries a popularity
score from 0 to 100. It answers "is anyone using this?" from playback history,
so you can spot candidates for cleanup. It never deletes or changes anything.

The score counts **active days**: UTC calendar days with at least one playback
session. Five sessions on one day count as one active day. Each active day
weighs less as it ages, halving every 180 days:

```text
weight     = 2^(-age_days / 180)       age_days: whole UTC days before today
activity   = sum of weights over all distinct active days
popularity = round(100 * (1 - e^(-activity / 5)))
```

`round` rounds halves away from zero. A day in the future (clock skew) counts
as today. The score is computed on every request against the current UTC
date, so it decays day by day even when no new playback sync runs. It is never
stored.

| History | Score |
| --- | --- |
| No recorded playback | 0 |
| One active day today | 18 |
| One active day 180 days ago | 10 |
| One active day 360 days ago | 5 |
| Three active days this week | about 45 |
| Ten active days in the last two weeks | about 86 |
| One active day three years ago | 0 |

The score ignores watch time, disk size, media age, and how many sessions
happened on a day. Movies, series, and artists use the same formula; for a
series every episode play counts toward the series, for an artist every track.
Plays without a known user count like any other.

It only knows what Tautulli recorded. A score of 0 on media older than your
Tautulli install means "no recorded playback", not proof that nobody watched
it. Without a configured and successfully synced playback source the score is
`null` (unavailable), never 0.

Aggregate playback imported before individual sessions were stored (the legacy
snapshots) has no per-session dates. Such an aggregate with at least one play
and a last-played time contributes one synthetic active day on that date,
decaying like any other day, so it adds at most 18 points and fades over time.
It merges with a real active day on the same date, and aggregates without a
last-played time contribute nothing. Sessions already covered by a legacy
aggregate are not counted again.

### Reclaim score

Alongside popularity, every movie, series, and artist carries a reclaim score
from 0 to 100. It answers "how attractive is this to delete to free disk
space?": 0 is a poor candidate, 100 an excellent one. It is advisory only.
Unpopularr never deletes or changes anything.

Popularity alone is not enough to rank deletions. It ignores size and age, so
an episode added yesterday, a 200 MB album, and an untouched 80 GiB remux can
all score 0. The reclaim score combines four factors, and all of them have to
agree before it gets high:

```text
unpopularity = 1 - popularity / 100
idle         = 1                                   if playCount == 0
               1 - 2^(-days_since_last_play / 180) otherwise
maturity     = 1 - 2^(-age_days / 180)
size_gib     = sizeOnDiskBytes / 2^30
space_value  = size_gib / (size_gib + 5)
reclaim      = round(100 * unpopularity * idle * maturity * space_value)
```

- **Idle** protects recently played media. Anything played today scores 0,
  however large it is. Protection halves every 180 days without a play.
- **Maturity** protects new arrivals. Media added today scores 0, even if it
  was never played. Protection halves every 180 days in the library, measured
  from `addedAt` (see [Media age](#media-age)).
- **Space value** saturates around 5 GiB: 5 GiB counts half, 50 GiB counts
  0.91, 500 GiB counts 0.99. Large items rank high, but a 500 GiB item does not
  count ten times more than a 50 GiB one. Small files never become top
  candidates just because they are old and unused.

Days are whole UTC calendar days, as for popularity. Timestamps in the future
(clock skew) count as today. `round` rounds halves away from zero, and the
result is clamped to 0–100. Like popularity, the score is computed on every
request and never stored.

| Item (popularity 0 unless noted) | Score |
| --- | --- |
| Never played, added today, 50 GiB | 0 |
| Never played, added 30 days ago, 50 GiB | 10 |
| Never played, added 180 days ago, 50 GiB | 45 |
| Never played, two years old, 5 GiB | 47 |
| Never played, two years old, 50 GiB | 85 |
| Never played, two years old, 500 GiB | 93 |
| Never played, five years old, 100 MiB | 2 |
| Popularity 15, last played 30 days ago, two years old, 50 GiB | 8 |
| Popularity 5, last played a year ago, three years old, 40 GiB | 63 |
| Played today, any age and size | 0 |

The score is `null` (unknown) rather than a guess when an input is missing:

- playback is unavailable, as for `popularityScore`
- `addedAt` is unknown
- playback reports plays but no last-played time (some legacy aggregates)

Never-played content with known playback counts as fully idle. As with
popularity, "never played" means "no playback Tautulli recorded", so media
older than your Tautulli history can look more idle than it really is.

## Web UI

The binary also serves a small web UI at the same address as the API. Open the
bind address (for example `http://127.0.0.1:3000`) in a browser. It has a
catalog view for finding large, never-played content and an activity view for
running and monitoring syncs.

The UI has no authentication of its own; the trusted-network or authenticating
reverse-proxy guidance above applies to it as well.

The compiled frontend in `web/dist` is embedded into the binary at build time,
the same way migrations are embedded. A fresh checkout ships only a placeholder,
so the UI routes report `frontend not built` until you build it (see
[Development](#development)). The API is unaffected when the UI is absent.

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
    "addedAt": "2024-03-01T00:00:00Z",
    "tmdbId": 123,
    "year": 2024,
    "playback": {
      "playCount": 4,
      "playDurationSeconds": 7200,
      "lastPlayedAt": "2026-06-24T12:00:00Z"
    },
    "popularityScore": 22,
    "reclaimScore": 0
  }
]
```

`playback` is `null` when playback collection is disabled or no playback sync
has completed successfully. After a successful sync, content with no matched
history has zero counts and a `null` `lastPlayedAt`.

`popularityScore` follows the same rule: `null` while playback is unavailable,
otherwise an integer from 0 to 100 (see [Popularity score](#popularity-score)).
The movie, series, and artist detail endpoints (`/api/v1/movies/{tmdbId}`,
`/api/v1/series/{tvdbId}`, `/api/v1/artists/{musicBrainzId}`) return the same
`popularityScore` for the same item.

`reclaimScore` is an integer from 0 to 100, or `null` when playback is
unavailable, `addedAt` is unknown, or plays have no last-played time (see
[Reclaim score](#reclaim-score)). The example scores 0 because 123456 bytes is
too small to be worth reclaiming. The detail endpoints return the same
`reclaimScore` for the same item.

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
cargo fix --allow-dirty --allow-staged
cargo fmt
cargo clippy --all-targets --all-features -- -D warnings
cargo test --all-targets --all-features
```

Database migrations are embedded from `migrations/` and applied at startup.
Add a new migration instead of modifying a migration that has shipped.

### Frontend

The frontend lives in `web/` (Vite + React + TypeScript). For a production
build, compile it before the binary so the assets are embedded:

```sh
npm --prefix web install
npm --prefix web run build
cargo build --release
```

For frontend development, run the API and the Vite dev server in separate
terminals. The dev server proxies `/api` to the backend, so no CORS
configuration is needed:

```sh
cargo run                 # API on 127.0.0.1:3000
npm --prefix web run dev      # UI on http://localhost:5173
```

Frontend checks (kept separate from the Rust gates so the Rust build needs no
Node toolchain):

```sh
npm --prefix web run typecheck
npm --prefix web run test
```
