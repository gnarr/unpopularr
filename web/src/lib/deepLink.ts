// Builds "open in the *arr web UI" deep links. The route path differs per
// service: Radarr routes movies by TMDB id, Lidarr artists by MusicBrainz id,
// and Sonarr series by title slug.
import type { ContentType, InstanceKind, InstanceLink } from '../api/types'

export interface LinkTarget {
  contentType: ContentType
  tmdbId?: number
  titleSlug?: string
  musicBrainzId?: string
}

// The path (relative to an instance's base URL) that the *arr web UI routes on,
// or null when the required id isn't available (e.g. a series synced before the
// slug was captured) — in which case no deep link should be rendered.
export function deepLinkPath(target: LinkTarget): string | null {
  switch (target.contentType) {
    case 'movie':
      return target.tmdbId != null ? `movie/${target.tmdbId}` : null
    case 'series':
      return target.titleSlug ? `series/${encodeURIComponent(target.titleSlug)}` : null
    case 'artist':
      return target.musicBrainzId ? `artist/${encodeURIComponent(target.musicBrainzId)}` : null
  }
}

// Absolute deep-link URL for one instance, or null when no link can be built.
export function deepLinkHref(instance: InstanceLink, target: LinkTarget): string | null {
  const path = deepLinkPath(target)
  if (path === null) return null
  try {
    // externalUrl always carries a trailing slash, so join treats it as a base.
    return new URL(path, instance.externalUrl).toString()
  } catch {
    return null
  }
}

const ARR_NAMES: Record<InstanceKind, string> = {
  sonarr: 'Sonarr',
  radarr: 'Radarr',
  lidarr: 'Lidarr',
}

export function arrName(kind: InstanceKind): string {
  return ARR_NAMES[kind]
}
