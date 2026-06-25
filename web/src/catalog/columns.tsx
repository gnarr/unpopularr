import { createColumnHelper, type ColumnDef, type RowData } from '@tanstack/react-table'
import type { ContentItem } from '../api/types'
import { detailCount, detailLabel, isNeverPlayed, year } from '../lib/content'
import { absoluteTime, formatBytes, formatDuration, relativeTime } from '../lib/format'
import { TypeBadge } from '../components/TypeBadge'
import { InstanceChips } from '../components/InstanceChips'

declare module '@tanstack/react-table' {
  // eslint-disable-next-line @typescript-eslint/no-unused-vars
  interface ColumnMeta<TData extends RowData, TValue> {
    align?: 'right'
  }
}

const helper = createColumnHelper<ContentItem>()

export function buildColumns(hasPlayback: boolean): Array<ColumnDef<ContentItem, any>> {
  const base: Array<ColumnDef<ContentItem, any>> = [
    helper.accessor('contentType', {
      header: 'Type',
      cell: (ctx) => <TypeBadge type={ctx.getValue()} />,
      sortingFn: 'text',
    }),
    helper.accessor('displayName', {
      header: 'Name',
      cell: (ctx) => {
        const item = ctx.row.original
        const releaseYear = year(item)
        return (
          <div className="flex items-center gap-2">
            <span className="font-medium text-slate-100">{item.displayName}</span>
            {releaseYear !== null && <span className="text-slate-500">({releaseYear})</span>}
          </div>
        )
      },
      sortingFn: (a, b) =>
        a.original.displayName
          .toLowerCase()
          .localeCompare(b.original.displayName.toLowerCase()),
    }),
    helper.accessor('sizeOnDiskBytes', {
      header: 'Size',
      cell: (ctx) => <span className="tabular-nums">{formatBytes(ctx.getValue())}</span>,
      meta: { align: 'right' },
    }),
    helper.accessor('fileCount', {
      header: 'Files',
      cell: (ctx) => <span className="tabular-nums">{ctx.getValue().toLocaleString()}</span>,
      meta: { align: 'right' },
    }),
    helper.accessor((item) => detailCount(item) ?? -1, {
      id: 'detail',
      header: 'Seasons / Albums',
      cell: (ctx) => <span className="text-slate-400">{detailLabel(ctx.row.original)}</span>,
      meta: { align: 'right' },
    }),
    helper.display({
      id: 'instances',
      header: 'Instances',
      cell: (ctx) => <InstanceChips instances={ctx.row.original.instances} />,
    }),
  ]

  if (!hasPlayback) return base

  return [
    ...base,
    helper.accessor((item) => item.playback?.playCount ?? undefined, {
      id: 'plays',
      header: 'Plays',
      sortUndefined: 'last',
      meta: { align: 'right' },
      cell: (ctx) => {
        const item = ctx.row.original
        if (item.playback === null) return <span className="text-slate-600">—</span>
        if (isNeverPlayed(item)) {
          return (
            <span className="rounded bg-red-500/15 px-1.5 py-0.5 text-xs font-medium text-red-300">
              Never
            </span>
          )
        }
        return <span className="tabular-nums">{item.playback.playCount.toLocaleString()}</span>
      },
    }),
    helper.accessor((item) => item.playback?.playDurationSeconds ?? undefined, {
      id: 'watchTime',
      header: 'Watch time',
      sortUndefined: 'last',
      meta: { align: 'right' },
      cell: (ctx) => {
        const item = ctx.row.original
        if (item.playback === null) return <span className="text-slate-600">—</span>
        if (isNeverPlayed(item)) return <span className="text-red-300">Never</span>
        return (
          <span className="tabular-nums">
            {formatDuration(item.playback.playDurationSeconds)}
          </span>
        )
      },
    }),
    helper.accessor(
      (item) =>
        item.playback?.lastPlayedAt ? new Date(item.playback.lastPlayedAt).getTime() : undefined,
      {
        id: 'lastPlayed',
        header: 'Last played',
        sortUndefined: 'last',
        meta: { align: 'right' },
        cell: (ctx) => {
          const item = ctx.row.original
          if (item.playback === null) return <span className="text-slate-600">—</span>
          if (item.playback.playCount === 0) return <span className="text-red-300">Never</span>
          if (item.playback.lastPlayedAt === null) return <span className="text-slate-500">—</span>
          return (
            <span title={absoluteTime(item.playback.lastPlayedAt)}>
              {relativeTime(item.playback.lastPlayedAt)}
            </span>
          )
        },
      },
    ),
  ]
}
