import type { MonthlyPlayback } from '../api/types'
import { formatDuration } from '../lib/format'

// One bar in the movie chart: a single calendar month on a continuous axis.
export interface MonthBar {
  month: string // YYYY-MM (UTC)
  label: string // e.g. "Jan 2024"
  minutes: number
  playCount: number
  // 0–100, this month's share of the busiest month.
  heightPercent: number
  tooltip: string
}

export interface MoviePlaybackChart {
  bars: MonthBar[]
  // False when there's no month to anchor the axis, or nothing was ever played.
  hasData: boolean
}

// Defensive cap so a garbage availability date can't spin a runaway axis; the
// visible window is at most this many months, anchored at the end.
const MAX_MONTHS = 240

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

// Build a continuous month axis from the movie's availability (or its first
// played month, before a re-sync fills `availableAt`) through the current month,
// filling months with no plays as zero-height bars.
export function buildMoviePlaybackChart(
  monthlyPlayback: MonthlyPlayback[],
  availableAt: string | null,
  now: number = Date.now(),
): MoviePlaybackChart {
  const byMonth = new Map(monthlyPlayback.map((entry) => [entry.month, entry]))
  const playedMonths = monthlyPlayback.map((entry) => entry.month).filter(isMonthKey)
  const availableMonth = availableAt ? monthKeyOf(new Date(availableAt)) : null

  const starts = [availableMonth, playedMonths.at(0)].filter(
    (month): month is string => month != null,
  )
  if (starts.length === 0) return { bars: [], hasData: false }

  const currentMonth = monthKeyOf(new Date(now))
  const lastPlayed = playedMonths.at(-1)
  const startIndex = Math.min(...starts.map(toMonthIndex))
  const endIndex = Math.max(toMonthIndex(currentMonth), lastPlayed ? toMonthIndex(lastPlayed) : 0)
  const clampedStart = Math.max(startIndex, endIndex - (MAX_MONTHS - 1))
  const from = Math.min(clampedStart, endIndex)

  const maxSeconds = Math.max(
    0,
    ...monthlyPlayback.map((entry) => entry.playDurationSeconds),
  )
  const bars: MonthBar[] = []
  for (let index = from; index <= endIndex; index += 1) {
    const month = fromMonthIndex(index)
    const entry = byMonth.get(month)
    const seconds = entry?.playDurationSeconds ?? 0
    const playCount = entry?.playCount ?? 0
    const label = monthLabel(month)
    bars.push({
      month,
      label,
      minutes: seconds / 60,
      playCount,
      heightPercent: maxSeconds > 0 ? (seconds / maxSeconds) * 100 : 0,
      tooltip:
        seconds > 0
          ? `${label} · ${formatDuration(seconds)} · ${playCount} ${playCount === 1 ? 'play' : 'plays'}`
          : `${label} · no plays`,
    })
  }

  return { bars, hasData: maxSeconds > 0 }
}

function isMonthKey(value: string): boolean {
  return /^\d{4}-\d{2}$/.test(value)
}

// A month as a single integer (year * 12 + zero-based month) for easy iteration.
function toMonthIndex(month: string): number {
  const [year, monthNumber] = month.split('-').map(Number)
  return year * 12 + (monthNumber - 1)
}

function fromMonthIndex(index: number): string {
  const year = Math.floor(index / 12)
  const monthNumber = (index % 12) + 1
  return `${year}-${String(monthNumber).padStart(2, '0')}`
}

function monthKeyOf(date: Date): string {
  const year = date.getUTCFullYear()
  const monthNumber = date.getUTCMonth() + 1
  return `${year}-${String(monthNumber).padStart(2, '0')}`
}

function monthLabel(month: string): string {
  const [year, monthNumber] = month.split('-').map(Number)
  return `${MONTH_NAMES[monthNumber - 1]} ${year}`
}
