// Builds "open in the *arr web UI" deep links. The backend supplies a
// per-instance, per-item route path (e.g. `movie/inception-27205`) on each
// instance reference; here we only join it onto that instance's external URL.
import type { InstanceKind } from '../api/types'

// Absolute deep-link URL, or null when the instance has no route path (e.g. a
// snapshot synced before the slug was captured) or the URL can't be built.
export function deepLinkHref(externalUrl: string, path: string | null): string | null {
  if (!path) return null
  try {
    // externalUrl always carries a trailing slash, so join treats it as a base.
    return new URL(path, externalUrl).toString()
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
