import type { InstanceReference } from '../api/types'
import { useInstanceMap } from '../api/queries'
import { arrName, deepLinkHref } from '../lib/deepLink'
import { absoluteTime, relativeTime } from '../lib/format'
import { ArrLinkIcon } from './ArrLinkIcon'

export function InstanceChips({ instances }: { instances: InstanceReference[] }) {
  const instanceMap = useInstanceMap()

  return (
    <div className="flex flex-wrap gap-1">
      {instances.map((instance) => {
        const link = instanceMap.get(instance.id)
        const href = link ? deepLinkHref(link.externalUrl, instance.deepLinkPath) : null
        return (
          <span
            key={instance.id}
            title={`Last synced ${relativeTime(instance.lastSuccessfulSyncAt)} · ${absoluteTime(instance.lastSuccessfulSyncAt)}`}
            className="inline-flex items-center gap-1 rounded bg-slate-700/60 px-1.5 py-0.5 text-xs text-slate-300"
          >
            {instance.name}
            {href && link && <ArrLinkIcon href={href} label={`Open in ${arrName(link.kind)}`} />}
          </span>
        )
      })}
    </div>
  )
}
