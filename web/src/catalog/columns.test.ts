import { describe, expect, it } from 'vitest'
import {
  createTable,
  getCoreRowModel,
  getSortedRowModel,
  type SortingState,
} from '@tanstack/react-table'
import type { MovieItem } from '../api/types'
import { buildColumns } from './columns'

function movie(tmdbId: number, popularityScore: number | null): MovieItem {
  return {
    contentType: 'movie',
    displayName: `Movie ${tmdbId}`,
    sizeOnDiskBytes: 100,
    fileCount: 1,
    instances: [],
    addedAt: null,
    playback: { playCount: 0, playDurationSeconds: 0, lastPlayedAt: null },
    popularityScore,
    tmdbId,
    year: 2020,
  }
}

// Drives the real column definitions through react-table's sorted row model
// without rendering, so the test covers the accessor and `sortUndefined`.
function sortedScores(items: MovieItem[], sorting: SortingState): Array<number | null> {
  const table = createTable({
    data: items,
    columns: buildColumns(true),
    state: {},
    onStateChange: () => {},
    renderFallbackValue: null,
    getCoreRowModel: getCoreRowModel(),
    getSortedRowModel: getSortedRowModel(),
  })
  table.setOptions((prev) => ({ ...prev, state: { ...table.initialState, sorting } }))
  return table.getRowModel().rows.map((row) => row.original.popularityScore)
}

describe('popularity column', () => {
  const items = [movie(1, 40), movie(2, null), movie(3, 0), movie(4, 90)]

  it('sorts ascending with unavailable scores last', () => {
    expect(sortedScores(items, [{ id: 'popularity', desc: false }])).toEqual([0, 40, 90, null])
  })

  it('sorts descending with unavailable scores still last', () => {
    expect(sortedScores(items, [{ id: 'popularity', desc: true }])).toEqual([90, 40, 0, null])
  })

  it('only exists when playback data is available', () => {
    const ids = (hasPlayback: boolean) => buildColumns(hasPlayback).map((column) => column.id)
    expect(ids(true)).toContain('popularity')
    expect(ids(false)).not.toContain('popularity')
  })
})
