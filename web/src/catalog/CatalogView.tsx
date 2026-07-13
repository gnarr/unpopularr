import { useMemo } from 'react'
import { useNavigate } from 'react-router'
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
import { useCatalogSettings } from './catalogSettings'

export function CatalogView() {
  const navigate = useNavigate()
  const content = useContent()
  const [settings, update, resetFilters] = useCatalogSettings()

  const items = useMemo(() => content.data ?? [], [content.data])
  const hasPlayback = useMemo(() => playbackAvailable(items), [items])

  const types = useMemo(() => new Set<ContentType>(settings.types), [settings.types])
  const instanceIds = useMemo(() => new Set(settings.instanceIds), [settings.instanceIds])

  const filtered = useMemo(() => {
    const filters: CatalogFilters = {
      search: settings.search,
      types,
      instanceIds,
      neverPlayedOnly: settings.neverPlayedOnly,
    }
    return items.filter((item) => matchesFilters(item, filters))
  }, [items, settings.search, types, instanceIds, settings.neverPlayedOnly])

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
        action={<Button onClick={() => navigate('/activity')}>Go to Activity</Button>}
      />
    )
  }

  return (
    <div className="space-y-6">
      <CatalogStats items={items} hasPlayback={hasPlayback} />
      <CatalogToolbar
        items={items}
        search={settings.search}
        onSearch={(value) => update({ search: value })}
        types={types}
        onTypes={(value) => update({ types: [...value] })}
        instanceIds={instanceIds}
        onInstanceIds={(value) => update({ instanceIds: [...value] })}
        neverPlayedOnly={settings.neverPlayedOnly}
        onNeverPlayedOnly={(value) => update({ neverPlayedOnly: value })}
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
          action={<Button onClick={resetFilters}>Clear filters</Button>}
        />
      ) : (
        <CatalogTable
          items={filtered}
          hasPlayback={hasPlayback}
          sorting={settings.sorting}
          onSortingChange={(sorting) => update({ sorting })}
        />
      )}
    </div>
  )
}
