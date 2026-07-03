import { useMemo } from 'react'
import type { ContentItem } from '../api/types'
import { isNeverPlayed } from '../lib/content'
import { formatBytes } from '../lib/format'
import { StatCard } from '../components/StatCard'

export function CatalogStats({
  items,
  hasPlayback,
}: {
  items: ContentItem[]
  hasPlayback: boolean
}) {
  const stats = useMemo(() => {
    let totalSize = 0
    let neverPlayedCount = 0
    let reclaimable = 0
    for (const item of items) {
      totalSize += item.sizeOnDiskBytes
      if (isNeverPlayed(item)) {
        neverPlayedCount += 1
        reclaimable += item.sizeOnDiskBytes
      }
    }
    return { count: items.length, totalSize, neverPlayedCount, reclaimable }
  }, [items])

  return (
    <div className="grid grid-cols-2 gap-4 sm:grid-cols-4">
      <StatCard label="Items" value={stats.count.toLocaleString()} />
      <StatCard label="Total size" value={formatBytes(stats.totalSize)} />
      {hasPlayback ? (
        <>
          <StatCard
            label="Never played"
            value={stats.neverPlayedCount.toLocaleString()}
            accent
          />
          <StatCard label="Reclaimable" value={formatBytes(stats.reclaimable)} accent />
        </>
      ) : (
        <div className="col-span-2 flex items-center rounded-lg border border-slate-800 bg-slate-900/40 px-4 py-3 text-sm text-slate-400">
          Connect Tautulli to surface never-played content.
        </div>
      )}
    </div>
  )
}
