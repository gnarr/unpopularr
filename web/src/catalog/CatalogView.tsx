import { useMemo, useState } from 'react'
import { useContent } from '../api/queries'
import type { ContentType } from '../api/types'
import { playbackAvailable } from '../lib/content'
import { matchesFilters, type CatalogFilters } from '../lib/filters'
import { Button } from '../components/Button'
import { EmptyState } from '../components/EmptyState'
import { StatCardSkeleton, TableSkeleton } from '../components/Skeletons'
import { CatalogStats } from './CatalogStats'
import { CatalogTable } from './CatalogTable'
import { CatalogToolbar } from './CatalogToolbar'

const ALL_TYPES: ContentType[] = ['movie', 'series', 'artist']

export function CatalogView({ onGoToActivity }: { onGoToActivity: () => void }) {
  const content = useContent()
  const [search, setSearch] = useState('')
  const [types, setTypes] = useState<Set<ContentType>>(() => new Set(ALL_TYPES))
  const [instanceIds, setInstanceIds] = useState<Set<string>>(() => new Set())
  const [neverPlayedOnly, setNeverPlayedOnly] = useState(false)

  const items = useMemo(() => content.data ?? [], [content.data])
  const hasPlayback = useMemo(() => playbackAvailable(items), [items])

  const filtered = useMemo(() => {
    const filters: CatalogFilters = { search, types, instanceIds, neverPlayedOnly }
    return items.filter((item) => matchesFilters(item, filters))
  }, [items, search, types, instanceIds, neverPlayedOnly])

  if (content.isPending) {
    return (
      <div className="space-y-6">
        <div className="grid grid-cols-2 gap-4 sm:grid-cols-4">
          {Array.from({ length: 4 }).map((_, index) => (
            <StatCardSkeleton key={index} />
          ))}
        </div>
        <TableSkeleton />
      </div>
    )
  }

  if (content.isError) {
    return (
      <EmptyState
        title="Couldn't load the catalog"
        description={content.error instanceof Error ? content.error.message : 'Unknown error'}
        action={<Button onClick={() => content.refetch()}>Retry</Button>}
      />
    )
  }

  if (items.length === 0) {
    return (
      <EmptyState
        title="No content imported yet"
        description="Run a library sync to import your Sonarr, Radarr, and Lidarr content."
        action={<Button onClick={onGoToActivity}>Go to Activity</Button>}
      />
    )
  }

  return (
    <div className="space-y-6">
      <CatalogStats items={items} hasPlayback={hasPlayback} />
      <CatalogToolbar
        items={items}
        search={search}
        onSearch={setSearch}
        types={types}
        onTypes={setTypes}
        instanceIds={instanceIds}
        onInstanceIds={setInstanceIds}
        neverPlayedOnly={neverPlayedOnly}
        onNeverPlayedOnly={setNeverPlayedOnly}
        hasPlayback={hasPlayback}
      />
      {!hasPlayback && (
        <div className="rounded-md border border-slate-800 bg-slate-900/50 px-4 py-3 text-sm text-slate-400">
          Playback data not available — configure a Tautulli source to identify never-played
          content.
        </div>
      )}
      {filtered.length === 0 ? (
        <EmptyState
          title="No items match your filters"
          action={
            <Button
              onClick={() => {
                setSearch('')
                setTypes(new Set(ALL_TYPES))
                setInstanceIds(new Set())
                setNeverPlayedOnly(false)
              }}
            >
              Clear filters
            </Button>
          }
        />
      ) : (
        <CatalogTable items={filtered} hasPlayback={hasPlayback} />
      )}
    </div>
  )
}
