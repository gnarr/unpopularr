import { useMemo, type ReactNode } from 'react'
import type { ContentItem, ContentType } from '../api/types'

const TYPE_LABELS: Record<ContentType, string> = {
  movie: 'Movies',
  series: 'Series',
  artist: 'Artists',
}

interface Props {
  items: ContentItem[]
  search: string
  onSearch: (value: string) => void
  types: Set<ContentType>
  onTypes: (value: Set<ContentType>) => void
  instanceIds: Set<string>
  onInstanceIds: (value: Set<string>) => void
  neverPlayedOnly: boolean
  onNeverPlayedOnly: (value: boolean) => void
  hasPlayback: boolean
}

export function CatalogToolbar(props: Props) {
  const instances = useMemo(() => {
    const byId = new Map<string, string>()
    for (const item of props.items) {
      for (const instance of item.instances) byId.set(instance.id, instance.name)
    }
    return [...byId.entries()]
      .map(([id, name]) => ({ id, name }))
      .sort((a, b) => a.name.localeCompare(b.name))
  }, [props.items])

  return (
    <div className="flex flex-wrap items-center gap-3">
      <input
        type="search"
        value={props.search}
        onChange={(event) => props.onSearch(event.target.value)}
        placeholder="Search by name…"
        className="w-full max-w-xs rounded-md border border-slate-700 bg-slate-900 px-3 py-1.5 text-sm text-slate-200 placeholder:text-slate-500 focus:border-indigo-500 focus:outline-none sm:w-64"
      />

      <ToggleGroup>
        {(Object.keys(TYPE_LABELS) as ContentType[]).map((type) => (
          <Toggle
            key={type}
            active={props.types.has(type)}
            onClick={() => props.onTypes(toggle(props.types, type))}
          >
            {TYPE_LABELS[type]}
          </Toggle>
        ))}
      </ToggleGroup>

      {instances.length > 1 && (
        <ToggleGroup>
          <Toggle active={props.instanceIds.size === 0} onClick={() => props.onInstanceIds(new Set())}>
            All
          </Toggle>
          {instances.map((instance) => (
            <Toggle
              key={instance.id}
              active={props.instanceIds.has(instance.id)}
              onClick={() => props.onInstanceIds(toggle(props.instanceIds, instance.id))}
            >
              {instance.name}
            </Toggle>
          ))}
        </ToggleGroup>
      )}

      <label
        className={`ml-auto inline-flex items-center gap-2 text-sm ${
          props.hasPlayback ? 'text-slate-300' : 'cursor-not-allowed text-slate-600'
        }`}
        title={props.hasPlayback ? undefined : 'Requires playback data'}
      >
        <input
          type="checkbox"
          disabled={!props.hasPlayback}
          checked={props.neverPlayedOnly}
          onChange={(event) => props.onNeverPlayedOnly(event.target.checked)}
          className="h-4 w-4 rounded border-slate-600 bg-slate-900 accent-indigo-500"
        />
        Never played only
      </label>
    </div>
  )
}

function toggle<T>(set: Set<T>, value: T): Set<T> {
  const next = new Set(set)
  if (next.has(value)) next.delete(value)
  else next.add(value)
  return next
}

function ToggleGroup({ children }: { children: ReactNode }) {
  return <div className="flex gap-1 rounded-md border border-slate-800 p-0.5">{children}</div>
}

function Toggle({
  active,
  onClick,
  children,
}: {
  active: boolean
  onClick: () => void
  children: ReactNode
}) {
  return (
    <button
      onClick={onClick}
      className={`rounded px-2 py-1 text-xs font-medium transition ${
        active ? 'bg-slate-700 text-slate-100' : 'text-slate-400 hover:text-slate-200'
      }`}
    >
      {children}
    </button>
  )
}
