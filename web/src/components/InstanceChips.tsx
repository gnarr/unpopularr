import type { InstanceReference } from '../api/types'
import { absoluteTime, relativeTime } from '../lib/format'

export function InstanceChips({ instances }: { instances: InstanceReference[] }) {
  return (
    <div className="flex flex-wrap gap-1">
      {instances.map((instance) => (
        <span
          key={instance.id}
          title={`Last synced ${relativeTime(instance.lastSuccessfulSyncAt)} · ${absoluteTime(instance.lastSuccessfulSyncAt)}`}
          className="rounded bg-slate-700/60 px-1.5 py-0.5 text-xs text-slate-300"
        >
          {instance.name}
        </span>
      ))}
    </div>
  )
}
