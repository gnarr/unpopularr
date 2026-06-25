import type { ContentItem, ContentType } from '../api/types'
import { isNeverPlayed } from './content'

export interface CatalogFilters {
  search: string
  types: Set<ContentType>
  instanceIds: Set<string> // empty means "all instances"
  neverPlayedOnly: boolean
}

export function matchesFilters(item: ContentItem, filters: CatalogFilters): boolean {
  if (!filters.types.has(item.contentType)) return false

  const search = filters.search.trim().toLowerCase()
  if (search.length > 0 && !item.displayName.toLowerCase().includes(search)) {
    return false
  }

  if (
    filters.instanceIds.size > 0 &&
    !item.instances.some((instance) => filters.instanceIds.has(instance.id))
  ) {
    return false
  }

  if (filters.neverPlayedOnly && !isNeverPlayed(item)) return false

  return true
}
