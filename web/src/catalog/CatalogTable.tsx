import { useMemo, useState } from 'react'
import {
  flexRender,
  getCoreRowModel,
  getSortedRowModel,
  useReactTable,
  type SortingState,
} from '@tanstack/react-table'
import type { ContentItem } from '../api/types'
import { isNeverPlayed, rowId } from '../lib/content'
import { buildColumns } from './columns'

export function CatalogTable({
  items,
  hasPlayback,
}: {
  items: ContentItem[]
  hasPlayback: boolean
}) {
  const columns = useMemo(() => buildColumns(hasPlayback), [hasPlayback])
  const [sorting, setSorting] = useState<SortingState>([
    { id: 'sizeOnDiskBytes', desc: true },
  ])

  const table = useReactTable({
    data: items,
    columns,
    state: { sorting },
    onSortingChange: setSorting,
    getRowId: rowId,
    getCoreRowModel: getCoreRowModel(),
    getSortedRowModel: getSortedRowModel(),
    enableMultiSort: true,
  })

  return (
    <div className="overflow-x-auto rounded-lg border border-slate-800">
      <table className="w-full border-separate border-spacing-0 text-sm">
        <thead className="sticky top-0 z-10 bg-slate-900/95 backdrop-blur">
          {table.getHeaderGroups().map((group) => (
            <tr key={group.id}>
              {group.headers.map((header) => {
                const align = header.column.columnDef.meta?.align
                const sorted = header.column.getIsSorted()
                const canSort = header.column.getCanSort()
                return (
                  <th
                    key={header.id}
                    aria-sort={
                      !canSort
                        ? undefined
                        : sorted === 'asc'
                          ? 'ascending'
                          : sorted === 'desc'
                            ? 'descending'
                            : 'none'
                    }
                    className={`select-none border-b border-slate-800 px-3 py-2 text-xs font-semibold uppercase tracking-wide text-slate-400 ${
                      align === 'right' ? 'text-right' : 'text-left'
                    }`}
                  >
                    <button
                      type="button"
                      disabled={!canSort}
                      onClick={canSort ? header.column.getToggleSortingHandler() : undefined}
                      className={`inline-flex items-center gap-1 rounded ${
                        canSort
                          ? 'cursor-pointer hover:text-slate-200 focus:outline-none focus-visible:ring-1 focus-visible:ring-indigo-400'
                          : ''
                      }`}
                    >
                      {flexRender(header.column.columnDef.header, header.getContext())}
                      {sorted === 'asc' ? '▲' : sorted === 'desc' ? '▼' : ''}
                    </button>
                  </th>
                )
              })}
            </tr>
          ))}
        </thead>
        <tbody>
          {table.getRowModel().rows.map((row) => {
            const neverPlayed = hasPlayback && isNeverPlayed(row.original)
            return (
              <tr
                key={row.id}
                className={`hover:bg-slate-800/30 ${
                  neverPlayed ? 'bg-red-500/[0.04]' : ''
                }`}
              >
                {row.getVisibleCells().map((cell) => {
                  const align = cell.column.columnDef.meta?.align
                  return (
                    <td
                      key={cell.id}
                      className={`border-b border-slate-800/60 px-3 py-1.5 ${align === 'right' ? 'text-right' : 'text-left'}`}
                    >
                      {flexRender(cell.column.columnDef.cell, cell.getContext())}
                    </td>
                  )
                })}
              </tr>
            )
          })}
        </tbody>
      </table>
      <div className="border-t border-slate-800 px-3 py-2 text-xs text-slate-500">
        {table.getRowModel().rows.length.toLocaleString()} items · shift-click a header to sort by
        multiple columns
      </div>
    </div>
  )
}
