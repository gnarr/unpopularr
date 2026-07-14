// Minimal Sonarr + Radarr + Lidarr + Tautulli mock backing the Playwright
// suite (port 39100). One series with a mix of episode states (watched, never
// watched, watched-then-deleted, unaired, and a legacy play without episode
// positions), one watched movie, and one artist with albums and a track play.
import { createServer } from 'node:http'

const series = [
  {
    id: 9,
    tvdbId: 7777,
    title: 'Mock Show',
    year: 2020,
    statistics: { episodeFileCount: 3, sizeOnDisk: 3000 },
    seasons: [
      { seasonNumber: 0, statistics: { episodeFileCount: 1 } },
      { seasonNumber: 1, statistics: { episodeFileCount: 2 } },
      { seasonNumber: 2, statistics: { episodeFileCount: 1 } },
    ],
  },
]

const episodes = [
  { seasonNumber: 0, episodeNumber: 1, title: 'Special', hasFile: true, episodeFile: { size: 111 } },
  { seasonNumber: 1, episodeNumber: 1, title: 'Pilot', airDateUtc: '2020-01-01T00:00:00Z', hasFile: true, episodeFile: { size: 1000 } },
  { seasonNumber: 1, episodeNumber: 2, title: 'Growth', airDateUtc: '2020-01-08T00:00:00Z', hasFile: true, episodeFile: { size: 1200 } },
  { seasonNumber: 1, episodeNumber: 3, title: 'Deleted One', airDateUtc: '2020-01-15T00:00:00Z', hasFile: false },
  { seasonNumber: 2, episodeNumber: 1, title: 'Return', airDateUtc: '2021-01-01T00:00:00Z', hasFile: true, episodeFile: { size: 800 } },
  { seasonNumber: 2, episodeNumber: 2, title: 'Unaired Finale', airDateUtc: '2099-01-01T00:00:00Z', hasFile: false },
]

const movies = [
  {
    id: 5,
    tmdbId: 550,
    title: 'Mock Movie',
    year: 1999,
    // Added long before the single Tautulli play, so the per-month plot spans
    // an availability window with one busy month and the rest empty.
    added: '2025-01-10T00:00:00Z',
    hasFile: true,
    sizeOnDisk: 2500,
    statistics: { movieFileCount: 1, sizeOnDisk: 2500 },
  },
]

const ARTIST_MBID = 'aaaaaaaa-1111-2222-3333-444444444444'
const artists = [
  {
    id: 3,
    artistName: 'Mock Artist',
    foreignArtistId: ARTIST_MBID,
    statistics: { trackFileCount: 12, sizeOnDisk: 4800 },
  },
]
const albums = [
  {
    artistId: 3,
    foreignAlbumId: 'bbbbbbbb-1111-2222-3333-444444444444',
    title: 'First Album',
    statistics: { trackFileCount: 10, sizeOnDisk: 4000 },
  },
  {
    artistId: 3,
    foreignAlbumId: 'cccccccc-1111-2222-3333-444444444444',
    title: 'Second Album',
    statistics: { trackFileCount: 2, sizeOnDisk: 800 },
  },
  // Zero files: the sync must skip it (never shown in the UI).
  {
    artistId: 3,
    foreignAlbumId: 'dddddddd-1111-2222-3333-444444444444',
    title: 'Empty Album',
    statistics: { trackFileCount: 0, sizeOnDisk: 0 },
  },
]

const history = {
  recordsFiltered: 5,
  data: [
    // Watched episode with positions.
    { media_type: 'episode', row_id: 1, grandparent_rating_key: 20, parent_media_index: 1, media_index: 1, play_duration: 1500, stopped: 1750000000, user_id: 1, friendly_name: 'Alice' },
    // Watched-then-deleted episode, watched by a second user.
    { media_type: 'episode', row_id: 2, grandparent_rating_key: 20, parent_media_index: 1, media_index: 3, play_duration: 1400, stopped: 1730000000, user_id: 2, friendly_name: 'Bob' },
    // Row without positions or user (pre-tracking shape) -> unattributed play.
    { media_type: 'episode', row_id: 3, grandparent_rating_key: 20, play_duration: 900, stopped: 1710000000 },
    // Movie plays match by the row's own rating key.
    { media_type: 'movie', row_id: 4, rating_key: 30, play_duration: 5400, stopped: 1745000000, user_id: 1, friendly_name: 'Alice' },
    // Track plays roll up to the artist via the grandparent rating key.
    { media_type: 'track', row_id: 5, grandparent_rating_key: 40, play_duration: 240, stopped: 1735000000, user_id: 1, friendly_name: 'Alice' },
  ],
}

const metadataByRatingKey = {
  20: { guid: 'plex://show/x', guids: ['tvdb://7777'] },
  30: { guid: 'plex://movie/x', guids: ['tmdb://550'] },
  40: { guid: 'plex://artist/x', guids: [`mbid://${ARTIST_MBID}`] },
}

const envelope = (data) => ({ response: { result: 'success', message: null, data } })

createServer((req, res) => {
  const url = new URL(req.url, 'http://localhost')
  const respond = (body) => {
    res.writeHead(200, { 'content-type': 'application/json' })
    res.end(JSON.stringify(body))
  }

  if (url.pathname === '/sonarr/api/v3/series') return respond(series)
  if (url.pathname === '/sonarr/api/v3/episode') {
    return respond(url.searchParams.get('seriesId') === '9' ? episodes : [])
  }
  if (url.pathname === '/radarr/api/v3/movie') return respond(movies)
  if (url.pathname === '/lidarr/api/v1/artist') return respond(artists)
  if (url.pathname === '/lidarr/api/v1/album') return respond(albums)
  if (url.pathname === '/taut/api/v2') {
    const cmd = url.searchParams.get('cmd')
    if (cmd === 'get_history') {
      const firstPage = url.searchParams.get('start') === '0'
      return respond(envelope(firstPage ? history : { recordsFiltered: 5, data: [] }))
    }
    if (cmd === 'get_metadata') {
      const metadata = metadataByRatingKey[url.searchParams.get('rating_key')]
      return respond(envelope(metadata ?? {}))
    }
  }
  res.writeHead(404, { 'content-type': 'application/json' })
  res.end('{}')
}).listen(39100, '127.0.0.1', () => console.log('mock Sonarr/Radarr/Lidarr/Tautulli listening on 39100'))
