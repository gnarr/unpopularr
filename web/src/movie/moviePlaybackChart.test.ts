import { describe, expect, it } from 'vitest'
import type { DailyPlayback } from '../api/types'
import { buildMoviePlaybackChart } from './moviePlaybackChart'

const at = (iso: string) => new Date(iso).getTime()
const NOW = at('2024-04-15T00:00:00Z')

describe('buildMoviePlaybackChart — month resolution', () => {
  it('fills a continuous month axis, sums within a month, and normalizes heights', () => {
    const daily: DailyPlayback[] = [
      { date: '2024-01-10', playCount: 1, playDurationSeconds: 3000 },
      { date: '2024-01-25', playCount: 1, playDurationSeconds: 600 }, // same month → summed
      { date: '2024-03-05', playCount: 1, playDurationSeconds: 1800 },
    ]
    const chart = buildMoviePlaybackChart(daily, '2024-01-05T00:00:00Z', 'month', NOW)

    expect(chart.bars.map((bar) => bar.key)).toEqual(['2024-01', '2024-02', '2024-03', '2024-04'])
    const jan = chart.bars[0]
    expect(jan.label).toBe('Jan 2024')
    expect(jan.minutes).toBe(60) // 3600 s
    expect(jan.playCount).toBe(2)
    expect(jan.heightPercent).toBe(100)
    expect(chart.bars[1].heightPercent).toBe(0) // Feb gap
    expect(chart.bars[2].heightPercent).toBe(50) // Mar, 1800 s
    expect(chart.hasData).toBe(true)
  })

  it('falls back to the first played month when availability is unknown', () => {
    const chart = buildMoviePlaybackChart(
      [{ date: '2024-02-15', playCount: 1, playDurationSeconds: 600 }],
      null,
      'month',
      NOW,
    )
    expect(chart.bars.map((bar) => bar.key)).toEqual(['2024-02', '2024-03', '2024-04'])
    expect(chart.hasData).toBe(true)
  })

  it('reports no data without an anchor month, and empty tooltips read plainly', () => {
    expect(buildMoviePlaybackChart([], null, 'month', NOW)).toEqual({ bars: [], hasData: false })

    const unplayed = buildMoviePlaybackChart([], '2024-02-01T00:00:00Z', 'month', NOW)
    expect(unplayed.hasData).toBe(false)
    expect(unplayed.bars.map((bar) => bar.key)).toEqual(['2024-02', '2024-03', '2024-04'])
    expect(unplayed.bars[0].tooltip).toBe('Feb 2024 · no plays')
  })
})

describe('buildMoviePlaybackChart — finer and coarser resolutions', () => {
  it('buckets by day', () => {
    const chart = buildMoviePlaybackChart(
      [
        { date: '2024-04-10', playCount: 1, playDurationSeconds: 600 },
        { date: '2024-04-12', playCount: 1, playDurationSeconds: 1200 },
      ],
      '2024-04-09T00:00:00Z',
      'day',
      at('2024-04-13T00:00:00Z'),
    )
    expect(chart.bars.map((bar) => bar.key)).toEqual([
      '2024-04-09',
      '2024-04-10',
      '2024-04-11',
      '2024-04-12',
      '2024-04-13',
    ])
    expect(chart.bars[1].label).toBe('Apr 10, 2024')
    expect(chart.bars[1].heightPercent).toBe(50)
    expect(chart.bars[3].tooltip).toBe('Apr 12, 2024 · 20m 0s · 1 play')
  })

  it('buckets days into ISO (Monday-start) weeks', () => {
    // Apr 2024 Mondays: 1, 8, 15. Apr 10 = Wed (week of the 8th); Apr 16 = Tue
    // (week of the 15th).
    const chart = buildMoviePlaybackChart(
      [
        { date: '2024-04-10', playCount: 1, playDurationSeconds: 600 },
        { date: '2024-04-16', playCount: 1, playDurationSeconds: 1200 },
      ],
      '2024-04-08T00:00:00Z',
      'week',
      at('2024-04-20T00:00:00Z'),
    )
    expect(chart.bars.map((bar) => bar.key)).toEqual(['2024-04-08', '2024-04-15'])
    expect(chart.bars[0].label).toBe('Week of Apr 8, 2024')
    expect(chart.bars[0].heightPercent).toBe(50)
    expect(chart.bars[1].heightPercent).toBe(100)
  })

  it('buckets by year', () => {
    const chart = buildMoviePlaybackChart(
      [
        { date: '2023-06-01', playCount: 1, playDurationSeconds: 600 },
        { date: '2024-02-01', playCount: 2, playDurationSeconds: 1800 },
      ],
      '2023-01-01T00:00:00Z',
      'year',
      NOW,
    )
    expect(chart.bars.map((bar) => bar.key)).toEqual(['2023', '2024'])
    expect(chart.bars[1].label).toBe('2024')
    expect(chart.bars[1].playCount).toBe(2)
    expect(chart.bars[1].heightPercent).toBe(100)
  })

  it('caps the number of bars so a garbage-old availability date cannot run away', () => {
    const chart = buildMoviePlaybackChart([], '1000-01-01T00:00:00Z', 'month', NOW)
    expect(chart.bars.length).toBe(400)
    expect(chart.hasData).toBe(false)
    // The window ends at the current bucket regardless of the ancient anchor.
    expect(chart.bars.at(-1)?.key).toBe('2024-04')
  })
})
