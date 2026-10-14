import { describe, expect, it } from 'vitest'
import {
  createTable,
  getCoreRowModel,
  getSortedRowModel,
  type SortingState,
} from '@tanstack/react-table'
import type { MovieItem } from '../api/types'
import { buildColumns } from './columns'

type Scores = Pick<MovieItem, 'popularityScore' | 'reclaimScore'>

function movie(tmdbId: number, scores: Partial<Scores>): MovieItem {
  return {
    contentType: 'movie',
    displayName: `Movie ${tmdbId}`,
    sizeOnDiskBytes: 100,
    fileCount: 1,
    instances: [],
    addedAt: null,
    playback: { playCount: 0, playDurationSeconds: 0, lastPlayedAt: null },
    popularityScore: null,
    reclaimScore: null,
    tmdbId,
    year: 2020,
    ...scores,
  }
}

// Drives the real column definitions through react-table's sorted row model
// without rendering, so the test covers the accessor and `sortUndefined`.
function sortedScores(
  items: MovieItem[],
  sorting: SortingState,
  score: keyof Scores,
): Array<number | null> {
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
  return table.getRowModel().rows.map((row) => row.original[score])
}

const columnIds = (hasPlayback: boolean) => buildColumns(hasPlayback).map((column) => column.id)

describe('popularity column', () => {
  const items = [40, null, 0, 90].map((popularityScore, id) => movie(id, { popularityScore }))
  const sorted = (desc: boolean) =>
    sortedScores(items, [{ id: 'popularity', desc }], 'popularityScore')

  it('sorts ascending with unavailable scores last', () => {
    expect(sorted(false)).toEqual([0, 40, 90, null])
  })

  it('sorts descending with unavailable scores still last', () => {
    expect(sorted(true)).toEqual([90, 40, 0, null])
  })

  it('only exists when playback data is available', () => {
    expect(columnIds(true)).toContain('popularity')
    expect(columnIds(false)).not.toContain('popularity')
  })
})

describe('reclaim column', () => {
  const items = [12, null, 85, 0, null].map((reclaimScore, id) => movie(id, { reclaimScore }))
  const sorted = (desc: boolean) => sortedScores(items, [{ id: 'reclaim', desc }], 'reclaimScore')

  it('sorts ascending with unavailable scores last', () => {
    expect(sorted(false)).toEqual([0, 12, 85, null, null])
  })

  it('sorts descending with unavailable scores still last', () => {
    expect(sorted(true)).toEqual([85, 12, 0, null, null])
  })

  it('only exists when playback data is available', () => {
    expect(columnIds(true)).toContain('reclaim')
    expect(columnIds(false)).not.toContain('reclaim')
  })
})
