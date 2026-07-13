import { useCallback, useEffect, useState } from 'react'
import type { SortingState } from '@tanstack/react-table'
import type { ContentType } from '../api/types'

// The catalog's remembered UI state, in a JSON-serializable shape (arrays, not
// the `Set`s the toolbar/table work with — those are reconstructed at the
// component boundary). Persisted to localStorage so filters, search, and sort
// survive reloads and navigation.
export interface CatalogSettings {
  search: string
  // Empty array is a real, remembered choice (the user deselected every type).
  types: ContentType[]
  // Empty array means "all instances".
  instanceIds: string[]
  neverPlayedOnly: boolean
  sorting: SortingState
}

export const ALL_TYPES: ContentType[] = ['movie', 'series', 'artist']

// Column ids that can appear in `sorting`. The playback ids only exist when a
// playback source is configured, but persisting them is harmless: react-table
// ignores sort entries whose column isn't present, and they take effect again
// once playback data returns.
const SORTABLE_COLUMN_IDS = new Set<string>([
  'contentType',
  'displayName',
  'sizeOnDiskBytes',
  'fileCount',
  'detail',
  'plays',
  'watchTime',
  'lastPlayed',
])

export const DEFAULT_SETTINGS: CatalogSettings = {
  search: '',
  types: ALL_TYPES,
  instanceIds: [],
  neverPlayedOnly: false,
  sorting: [{ id: 'sizeOnDiskBytes', desc: true }],
}

export const STORAGE_KEY = 'unpopularr.catalog.settings'

const CONTENT_TYPES = new Set<string>(ALL_TYPES)

function parseSorting(value: unknown): SortingState {
  if (!Array.isArray(value)) return DEFAULT_SETTINGS.sorting
  return value.filter(
    (entry): entry is { id: string; desc: boolean } =>
      typeof entry === 'object' &&
      entry !== null &&
      typeof (entry as { id?: unknown }).id === 'string' &&
      SORTABLE_COLUMN_IDS.has((entry as { id: string }).id) &&
      typeof (entry as { desc?: unknown }).desc === 'boolean',
  )
}

// Tolerant of stale or corrupt storage: every field falls back to its default
// independently, so one bad field never discards the others.
export function parseCatalogSettings(raw: string | null): CatalogSettings {
  if (raw === null) return DEFAULT_SETTINGS

  let parsed: unknown
  try {
    parsed = JSON.parse(raw)
  } catch {
    return DEFAULT_SETTINGS
  }
  if (typeof parsed !== 'object' || parsed === null) return DEFAULT_SETTINGS

  const record = parsed as Record<string, unknown>

  return {
    search: typeof record.search === 'string' ? record.search : DEFAULT_SETTINGS.search,
    types: Array.isArray(record.types)
      ? (record.types.filter((type) => CONTENT_TYPES.has(type as string)) as ContentType[])
      : DEFAULT_SETTINGS.types,
    instanceIds: Array.isArray(record.instanceIds)
      ? record.instanceIds.filter((id): id is string => typeof id === 'string')
      : DEFAULT_SETTINGS.instanceIds,
    neverPlayedOnly:
      typeof record.neverPlayedOnly === 'boolean'
        ? record.neverPlayedOnly
        : DEFAULT_SETTINGS.neverPlayedOnly,
    sorting: parseSorting(record.sorting),
  }
}

export function serializeCatalogSettings(settings: CatalogSettings): string {
  return JSON.stringify(settings)
}

function safeGet(key: string): string | null {
  if (typeof window === 'undefined') return null
  try {
    return window.localStorage.getItem(key)
  } catch {
    return null
  }
}

function safeSet(key: string, value: string): void {
  if (typeof window === 'undefined') return
  try {
    window.localStorage.setItem(key, value)
  } catch {
    // Storage disabled or over quota — degrade to in-memory state.
  }
}

// Remembers the catalog's filter/search/sort state in localStorage. Returns the
// current settings, a partial-update setter, and a filter reset that leaves the
// sort untouched (matching the toolbar's "Clear filters" behavior).
export function useCatalogSettings(): [
  CatalogSettings,
  (patch: Partial<CatalogSettings>) => void,
  () => void,
] {
  const [settings, setSettings] = useState<CatalogSettings>(() =>
    parseCatalogSettings(safeGet(STORAGE_KEY)),
  )

  useEffect(() => {
    safeSet(STORAGE_KEY, serializeCatalogSettings(settings))
  }, [settings])

  const update = useCallback((patch: Partial<CatalogSettings>) => {
    setSettings((prev) => ({ ...prev, ...patch }))
  }, [])

  const resetFilters = useCallback(() => {
    update({
      search: '',
      types: ALL_TYPES,
      instanceIds: [],
      neverPlayedOnly: false,
    })
  }, [update])

  return [settings, update, resetFilters]
}
