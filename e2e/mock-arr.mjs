// Minimal Sonarr + Tautulli mock backing the Playwright suite (port 39100).
// One series with a mix of episode states: watched, never watched,
// watched-then-deleted, unaired, and a legacy play without episode positions.
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

const history = {
  recordsFiltered: 3,
  data: [
    // Watched episode with positions.
    { media_type: 'episode', row_id: 1, grandparent_rating_key: 20, parent_media_index: 1, media_index: 1, play_duration: 1500, stopped: 1750000000 },
    // Watched-then-deleted episode.
    { media_type: 'episode', row_id: 2, grandparent_rating_key: 20, parent_media_index: 1, media_index: 3, play_duration: 1400, stopped: 1730000000 },
    // Row without positions (pre-tracking shape) -> unattributed play.
    { media_type: 'episode', row_id: 3, grandparent_rating_key: 20, play_duration: 900, stopped: 1710000000 },
  ],
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
  if (url.pathname === '/taut/api/v2') {
    const cmd = url.searchParams.get('cmd')
    if (cmd === 'get_history') {
      const firstPage = url.searchParams.get('start') === '0'
      return respond(envelope(firstPage ? history : { recordsFiltered: 3, data: [] }))
    }
    if (cmd === 'get_metadata') {
      return respond(envelope({ guid: 'plex://show/x', guids: ['tvdb://7777'] }))
    }
  }
  res.writeHead(404, { 'content-type': 'application/json' })
  res.end('{}')
}).listen(39100, '127.0.0.1', () => console.log('mock Sonarr/Tautulli listening on 39100'))
