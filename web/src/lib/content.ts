import type { ContentItem } from '../api/types'

export function stableId(item: ContentItem): string {
  switch (item.contentType) {
    case 'movie':
      return String(item.tmdbId)
    case 'series':
      return String(item.tvdbId)
    case 'artist':
      return item.musicBrainzId
  }
}

export function rowId(item: ContentItem): string {
  return `${item.contentType}:${stableId(item)}`
}

export function year(item: ContentItem): number | null {
  return item.contentType === 'artist' ? null : item.year
}

// Sortable count of the type-specific child unit; movies have none.
export function detailCount(item: ContentItem): number | null {
  switch (item.contentType) {
    case 'series':
      return item.seasonsWithFiles
    case 'artist':
      return item.albumsWithFiles
    case 'movie':
      return null
  }
}

export function detailLabel(item: ContentItem): string {
  switch (item.contentType) {
    case 'series':
      return `${item.seasonsWithFiles} ${item.seasonsWithFiles === 1 ? 'season' : 'seasons'}`
    case 'artist':
      return `${item.albumsWithFiles} ${item.albumsWithFiles === 1 ? 'album' : 'albums'}`
    case 'movie':
      return '—'
  }
}

// True only when playback data exists AND the item was never played. A `null`
// playback (unavailable) is NOT never-played — the distinction is load-bearing.
export function isNeverPlayed(item: ContentItem): boolean {
  return item.playback !== null && item.playback.playCount === 0
}

export function playbackAvailable(items: ContentItem[]): boolean {
  return items.some((item) => item.playback !== null)
}
