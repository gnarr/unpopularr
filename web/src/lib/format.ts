const BYTE_UNITS = ['B', 'KiB', 'MiB', 'GiB', 'TiB', 'PiB']

// Binary units, matching the Sonarr/Radarr convention.
export function formatBytes(bytes: number): string {
  if (!Number.isFinite(bytes) || bytes <= 0) return '0 B'
  const exponent = Math.min(
    Math.floor(Math.log(bytes) / Math.log(1024)),
    BYTE_UNITS.length - 1,
  )
  const value = bytes / 1024 ** exponent
  const rounded = exponent === 0 ? value : Math.round(value * 10) / 10
  return `${rounded} ${BYTE_UNITS[exponent]}`
}

export function formatDuration(seconds: number): string {
  if (!Number.isFinite(seconds) || seconds <= 0) return '0s'
  const total = Math.round(seconds)
  const hours = Math.floor(total / 3600)
  const minutes = Math.floor((total % 3600) / 60)
  const secs = total % 60
  if (hours > 0) return `${hours}h ${minutes}m`
  if (minutes > 0) return `${minutes}m ${secs}s`
  return `${secs}s`
}

const RELATIVE_UNITS: Array<[Intl.RelativeTimeFormatUnit, number]> = [
  ['year', 60 * 60 * 24 * 365],
  ['month', 60 * 60 * 24 * 30],
  ['day', 60 * 60 * 24],
  ['hour', 60 * 60],
  ['minute', 60],
  ['second', 1],
]

const relativeFormatter = new Intl.RelativeTimeFormat(undefined, { numeric: 'auto' })

export function relativeTime(iso: string, now: number = Date.now()): string {
  const timestamp = new Date(iso).getTime()
  if (Number.isNaN(timestamp)) return '—'
  const deltaSeconds = (timestamp - now) / 1000
  const absSeconds = Math.abs(deltaSeconds)
  for (const [unit, secondsInUnit] of RELATIVE_UNITS) {
    if (absSeconds >= secondsInUnit || unit === 'second') {
      return relativeFormatter.format(Math.round(deltaSeconds / secondsInUnit), unit)
    }
  }
  return '—'
}

const AGE_UNITS: Array<[string, number]> = [
  ['y', 60 * 60 * 24 * 365],
  ['mo', 60 * 60 * 24 * 30],
  ['d', 60 * 60 * 24],
  ['h', 60 * 60],
]

// How long ago, compactly: "2y", "7mo", "18d", "4h". `relativeTime`'s prose
// reads wrong in a right-aligned numeric column next to Size and Files.
export function formatAge(iso: string, now: number = Date.now()): string {
  const timestamp = new Date(iso).getTime()
  if (Number.isNaN(timestamp)) return '—'
  // A future date means clock skew or an *arr reporting one; it is not a
  // negative age.
  const seconds = Math.max(0, (now - timestamp) / 1000)
  for (const [suffix, secondsInUnit] of AGE_UNITS) {
    if (seconds >= secondsInUnit) return `${Math.floor(seconds / secondsInUnit)}${suffix}`
  }
  return '0d'
}

export function absoluteTime(iso: string): string {
  const date = new Date(iso)
  return Number.isNaN(date.getTime()) ? iso : date.toLocaleString()
}

export function elapsedSeconds(startedAt: string, completedAt: string): number {
  return (new Date(completedAt).getTime() - new Date(startedAt).getTime()) / 1000
}
