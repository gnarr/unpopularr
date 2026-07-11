import { describe, expect, it } from 'vitest'
import type { MonthlyPlayback } from '../api/types'
import { buildMoviePlaybackChart } from './moviePlaybackChart'

const at = (iso: string) => new Date(iso).getTime()
const NOW = at('2024-04-15T00:00:00Z')

describe('buildMoviePlaybackChart', () => {
  it('fills a continuous axis from availability to now and normalizes heights', () => {
    const monthly: MonthlyPlayback[] = [
      { month: '2024-01', playCount: 2, playDurationSeconds: 3600 },
      { month: '2024-03', playCount: 1, playDurationSeconds: 1800 },
    ]
    const chart = buildMoviePlaybackChart(monthly, '2024-01-10T00:00:00Z', NOW)

    expect(chart.bars.map((bar) => bar.month)).toEqual([
      '2024-01',
      '2024-02',
      '2024-03',
      '2024-04',
    ])
    expect(chart.bars[0].heightPercent).toBe(100)
    expect(chart.bars[1].heightPercent).toBe(0) // gap month
    expect(chart.bars[2].heightPercent).toBe(50)
    expect(chart.bars[0].minutes).toBe(60)
    expect(chart.hasData).toBe(true)
  })

  it('falls back to the first played month when availability is unknown', () => {
    const chart = buildMoviePlaybackChart(
      [{ month: '2024-02', playCount: 1, playDurationSeconds: 600 }],
      null,
      NOW,
    )
    expect(chart.bars.map((bar) => bar.month)).toEqual(['2024-02', '2024-03', '2024-04'])
    expect(chart.hasData).toBe(true)
  })

  it('builds readable labels and tooltips across a year boundary', () => {
    const chart = buildMoviePlaybackChart(
      [{ month: '2024-01', playCount: 1, playDurationSeconds: 3600 }],
      '2023-11-01T00:00:00Z',
      at('2024-02-15T00:00:00Z'),
    )
    expect(chart.bars.map((bar) => bar.month)).toEqual([
      '2023-11',
      '2023-12',
      '2024-01',
      '2024-02',
    ])
    const jan = chart.bars.find((bar) => bar.month === '2024-01')!
    expect(jan.label).toBe('Jan 2024')
    expect(jan.tooltip).toBe('Jan 2024 · 1h 0m · 1 play')
    // Empty months read explicitly as such.
    expect(chart.bars[0].label).toBe('Nov 2023')
    expect(chart.bars[0].tooltip).toBe('Nov 2023 · no plays')
  })

  it('reports no data without any anchor month, and stays flat when never played', () => {
    expect(buildMoviePlaybackChart([], null, NOW)).toEqual({ bars: [], hasData: false })

    const availableButUnplayed = buildMoviePlaybackChart(
      [],
      '2024-01-05T00:00:00Z',
      at('2024-03-15T00:00:00Z'),
    )
    expect(availableButUnplayed.hasData).toBe(false)
    expect(availableButUnplayed.bars.map((bar) => bar.month)).toEqual([
      '2024-01',
      '2024-02',
      '2024-03',
    ])
  })
})
