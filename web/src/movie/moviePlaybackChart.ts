import type { DailyPlayback } from '../api/types'
import { formatDuration } from '../lib/format'

export type Resolution = 'day' | 'week' | 'month' | 'year'

export const RESOLUTIONS: Resolution[] = ['day', 'week', 'month', 'year']

// One bar on the chart: a single day/week/month/year bucket on a continuous axis.
export interface ChartBar {
  // Stable bucket key (its start), unique within the chart.
  key: string
  label: string
  minutes: number
  playCount: number
  // 0–100, this bucket's share of the busiest visible bucket.
  heightPercent: number
  tooltip: string
}

export interface MoviePlaybackChart {
  bars: ChartBar[]
  // False when the visible window has no plays (all baseline). The section
  // still renders so the resolution toggle stays reachable.
  hasData: boolean
}

// Cap on rendered bars, uniform across resolutions: a coarse resolution shows
// decades, a fine one a bounded recent window. Guards the DOM and any garbage
// availability date; the oldest (leading) buckets are clipped first.
const MAX_BARS = 400

const MONTH_NAMES = [
  'Jan',
  'Feb',
  'Mar',
  'Apr',
  'May',
  'Jun',
  'Jul',
  'Aug',
  'Sep',
  'Oct',
  'Nov',
  'Dec',
]

// Build a continuous axis at the chosen resolution, from the movie's
// availability (or its first played bucket, before a re-sync fills `availableAt`)
// through the current bucket, filling empty buckets with zero-height bars.
export function buildMoviePlaybackChart(
  dailyPlayback: DailyPlayback[],
  availableAt: string | null,
  resolution: Resolution = 'month',
  now: number = Date.now(),
): MoviePlaybackChart {
  // Fold the per-day rows into the chosen bucket, tracking the earliest play.
  const totals = new Map<string, { seconds: number; count: number }>()
  let earliestPlayed: Date | null = null
  for (const entry of dailyPlayback) {
    const day = parseDay(entry.date)
    if (day === null) continue
    const start = bucketStart(day, resolution)
    const key = bucketKey(start, resolution)
    const bucket = totals.get(key)
    if (bucket) {
      bucket.seconds += entry.playDurationSeconds
      bucket.count += entry.playCount
    } else {
      totals.set(key, { seconds: entry.playDurationSeconds, count: entry.playCount })
    }
    if (earliestPlayed === null || start < earliestPlayed) earliestPlayed = start
  }

  const availableStart = availableAt ? bucketStart(new Date(availableAt), resolution) : null
  const anchors = [availableStart, earliestPlayed].filter((date): date is Date => date !== null)
  if (anchors.length === 0) return { bars: [], hasData: false }

  const end = bucketStart(new Date(now), resolution)
  const anchor = anchors.reduce((earliest, date) => (date < earliest ? date : earliest))
  const floor = subtractBuckets(end, resolution, MAX_BARS - 1)
  let start = anchor < floor ? floor : anchor
  if (start > end) start = end

  // Materialize the visible buckets, then scale to the busiest visible one so a
  // clipped older spike can't flatten the rest.
  const raw: Array<{ key: string; label: string; seconds: number; count: number }> = []
  for (let cursor = start; cursor <= end; cursor = addBucket(cursor, resolution)) {
    const key = bucketKey(cursor, resolution)
    const bucket = totals.get(key)
    raw.push({
      key,
      label: bucketLabel(cursor, resolution),
      seconds: bucket?.seconds ?? 0,
      count: bucket?.count ?? 0,
    })
  }

  const maxSeconds = Math.max(0, ...raw.map((entry) => entry.seconds))
  const bars = raw.map((entry) => ({
    key: entry.key,
    label: entry.label,
    minutes: entry.seconds / 60,
    playCount: entry.count,
    heightPercent: maxSeconds > 0 ? (entry.seconds / maxSeconds) * 100 : 0,
    tooltip:
      entry.count > 0
        ? `${entry.label} · ${formatDuration(entry.seconds)} · ${entry.count} ${entry.count === 1 ? 'play' : 'plays'}`
        : `${entry.label} · no plays`,
  }))

  return { bars, hasData: maxSeconds > 0 }
}

// Parse a `YYYY-MM-DD` day key to a UTC Date, or null if malformed.
function parseDay(date: string): Date | null {
  if (!/^\d{4}-\d{2}-\d{2}$/.test(date)) return null
  const [year, month, day] = date.split('-').map(Number)
  return new Date(Date.UTC(year, month - 1, day))
}

// The UTC start of the bucket containing `date`: the day, the Monday of its
// week, the first of its month, or January 1 of its year.
function bucketStart(date: Date, resolution: Resolution): Date {
  const year = date.getUTCFullYear()
  const month = date.getUTCMonth()
  const day = date.getUTCDate()
  if (resolution === 'year') return new Date(Date.UTC(year, 0, 1))
  if (resolution === 'month') return new Date(Date.UTC(year, month, 1))
  if (resolution === 'week') {
    // ISO weeks start Monday; getUTCDay is 0 (Sun) … 6 (Sat).
    const daysSinceMonday = (date.getUTCDay() + 6) % 7
    return new Date(Date.UTC(year, month, day - daysSinceMonday))
  }
  return new Date(Date.UTC(year, month, day))
}

function addBucket(date: Date, resolution: Resolution): Date {
  const year = date.getUTCFullYear()
  const month = date.getUTCMonth()
  const day = date.getUTCDate()
  if (resolution === 'year') return new Date(Date.UTC(year + 1, 0, 1))
  if (resolution === 'month') return new Date(Date.UTC(year, month + 1, 1))
  return new Date(Date.UTC(year, month, day + (resolution === 'week' ? 7 : 1)))
}

function subtractBuckets(date: Date, resolution: Resolution, count: number): Date {
  const year = date.getUTCFullYear()
  const month = date.getUTCMonth()
  const day = date.getUTCDate()
  if (resolution === 'year') return new Date(Date.UTC(year - count, 0, 1))
  if (resolution === 'month') return new Date(Date.UTC(year, month - count, 1))
  return new Date(Date.UTC(year, month, day - count * (resolution === 'week' ? 7 : 1)))
}

function bucketKey(date: Date, resolution: Resolution): string {
  const year = date.getUTCFullYear()
  const month = String(date.getUTCMonth() + 1).padStart(2, '0')
  const day = String(date.getUTCDate()).padStart(2, '0')
  if (resolution === 'year') return `${year}`
  if (resolution === 'month') return `${year}-${month}`
  return `${year}-${month}-${day}` // day and week both keyed by their start day
}

function bucketLabel(date: Date, resolution: Resolution): string {
  const year = date.getUTCFullYear()
  const monthName = MONTH_NAMES[date.getUTCMonth()]
  const day = date.getUTCDate()
  if (resolution === 'year') return `${year}`
  if (resolution === 'month') return `${monthName} ${year}`
  if (resolution === 'week') return `Week of ${monthName} ${day}, ${year}`
  return `${monthName} ${day}, ${year}`
}
